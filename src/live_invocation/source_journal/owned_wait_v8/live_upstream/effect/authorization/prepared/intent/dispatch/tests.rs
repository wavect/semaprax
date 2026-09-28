//! Real source/ACK/target path. No trusted raw ACK producer or caller ledger.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    Settlement, TargetHostError, TargetHostRequest, TargetResponseSink, TypedCarrier,
};
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
use crate::live_invocation::SourceInvocationClock;
use std::cell::Cell;
use std::sync::{Arc, Weak};
const RESULT: &[u8] =
    b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n";
struct Clock<'a> {
    cancel: &'a AgentCancellation,
    now: Cell<i64>,
    post: Cell<u8>,
}
impl crate::live_invocation::InvocationClock for Clock<'_> {
    fn now_millis(&self) -> i64 {
        match self.post.get() {
            1 => {
                self.cancel.cancel();
            }
            2 => panic!("post-host clock panic"),
            _ => (),
        }
        self.now.get()
    }
}
impl SourceInvocationClock for Clock<'_> {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
struct Host<'a> {
    calls: usize,
    request: Vec<u8>,
    result: Vec<u8>,
    clock: &'a Clock<'a>,
    mode: u8,
}
impl TargetHostHandler for Host<'_> {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        self.request = request.canonical_wire();
        assert_eq!(request.fuel(), 1);
        assert_eq!(request.turn(), 0);
        self.clock.post.set(self.mode);
        if self.mode == 2 {
            panic!("host panic remains primary");
        }
        self.result = TypedCarrier::new(request.operation().result_type(), RESULT.to_vec())
            .unwrap()
            .encode();
        sink.write(&self.result)
            .map_err(|_| TargetHostError::Failed)
    }
}
fn activated<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    clock: &'j Clock<'j>,
    policy: &'j CapabilityPolicy,
) -> (LiveActivatedOwnedEffectV8<'j>, Vec<Weak<[u8]>>, String) {
    let (ready, weak) = test_ready_obligation(journal, cancel, clock, policy);
    let ready = journal
        .begin_session()
        .unwrap()
        .append_owned_effect(ready)
        .unwrap_or_else(|_| panic!("Ready ACK"));
    let consumed = ready
        .advance_ready()
        .unwrap_or_else(|_| panic!("same Ready"));
    let consumed = journal
        .begin_session()
        .unwrap()
        .append_owned_authorization_consumed(consumed)
        .unwrap_or_else(|_| panic!("Consumed ACK"));
    let held = consumed
        .reserve_owned_reduce()
        .unwrap_or_else(|_| panic!("same exclusive hold"));
    let prepared = held
        .advance_authorization()
        .unwrap_or_else(|_| panic!("same Prepared"));
    let intent = prepared
        .prepare_intent()
        .unwrap_or_else(|_| panic!("Intent selection"));
    let EntryV8::Ordinary(
        crate::live_invocation::source_journal::SourceJournalEntry::EffectIntent {
            request_digest,
            ..
        },
    ) = intent.selected_row()
    else {
        panic!("Intent");
    };
    let digest = request_digest.clone();
    let envelope = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_intent(intent)
        .unwrap_or_else(|_| panic!("actual Intent ACK"));
    let activated = envelope
        .advance_intent()
        .unwrap_or_else(|_| panic!("actual activation"));
    assert_eq!(activated.accounting, TargetAccounting::default());
    (activated, weak, digest)
}
#[test]
fn owned_wait_live_dispatch_once_preserves_frozen_request_and_invocation_accounting() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let clock = Clock {
            cancel: &cancel,
            now: Cell::new(1),
            post: Cell::new(0),
        };
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (actual, weak, request_digest) = activated(&journal, &cancel, &clock, &policy);
        let sequence = actual.lineage.session.sequence();
        let mut host = Host {
            calls: 0,
            request: Vec::new(),
            result: Vec::new(),
            clock: &clock,
            mode: 0,
        };
        let staged = actual
            .dispatch(&mut host)
            .unwrap_or_else(|_| panic!("actual target"));
        assert_eq!(host.calls, 1);
        let evidence = staged.staged.dispatch().unwrap().evidence();
        assert_eq!(evidence.settlement(), Settlement::Returned);
        assert!(evidence.dispatched());
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"semaprax.agent-target-host.request.v2\0");
        hash.update(&host.request);
        let observed = "sha256:".to_owned()
            + &hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
        assert_eq!(observed, request_digest);
        evidence
            .replay_exchange_wire(&host.request, Some(&host.result))
            .unwrap();
        assert_eq!(staged.accounting.calls(), 1);
        assert_eq!(staged.accounting.fuel(), 1);
        assert_eq!(staged.accounting.request_bytes(), host.request.len() as u64);
        assert_eq!(staged.accounting.result_bytes(), host.result.len() as u64);
        assert_eq!(*staged.accounting(), evidence.accounting());
        assert!(staged.staged.observation().is_some());
        assert_eq!(
            staged.lineage.session.sequence(),
            sequence,
            "dispatch adds no fake settlement ACK"
        );
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        staged.validate_live().unwrap();
        drop(staged);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        assert!(
            journal.hold().is_err(),
            "one-use hold retired, no redispatch owner"
        );
    });
}
#[test]
fn owned_wait_live_dispatch_pre_entry_cancel_deadline_and_pins_are_zero_host() {
    for mode in 0..3 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let context = context.with_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = AgentCancellation::new();
                let clock = Clock {
                    cancel: &cancel,
                    now: Cell::new(1),
                    post: Cell::new(0),
                };
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let (actual, weak, _) = activated(&journal, &cancel, &clock, &policy);
                match mode {
                    0 => cancel.cancel(),
                    1 => clock.now.set(
                        journal
                            .context()
                            .ordinary()
                            .deadline_millis()
                            .checked_add(1)
                            .unwrap(),
                    ),
                    _ => {
                        let paths: Vec<_> = std::fs::read_dir(directory)
                            .unwrap()
                            .map(|e| e.unwrap().path())
                            .collect();
                        assert_eq!(paths.len(), 1, "one registered file");
                        std::fs::rename(&paths[0], directory.join("replaced")).unwrap();
                        std::fs::write(&paths[0], b"").unwrap();
                    }
                }
                let mut host = Host {
                    calls: 0,
                    request: Vec::new(),
                    result: Vec::new(),
                    clock: &clock,
                    mode: 0,
                };
                let failed = actual.dispatch(&mut host).err().expect("entry refused");
                let LiveEffectDispatchFailureV8::Before { _owner, error } = &failed else {
                    panic!("no target entry");
                };
                if mode == 1 {
                    assert_eq!(*error, SourceJournalError::Time);
                }
                assert_eq!(_owner.accounting, TargetAccounting::default());
                assert_eq!(host.calls, 0);
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert_eq!(_owner.validate_live(), Err(SourceJournalError::Poisoned));
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    }
}
#[test]
fn owned_wait_live_dispatch_post_host_false_cancel_and_panic_keep_primary_and_charges() {
    for mode in [1, 2] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let clock = Clock {
                cancel: &cancel,
                now: Cell::new(1),
                post: Cell::new(0),
            };
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (actual, weak, _) = activated(&journal, &cancel, &clock, &policy);
            let mut host = Host {
                calls: 0,
                request: Vec::new(),
                result: Vec::new(),
                clock: &clock,
                mode,
            };
            let failed = actual
                .dispatch(&mut host)
                .err()
                .expect("post-host quarantine");
            let LiveEffectDispatchFailureV8::After { _owner, error } = &failed else {
                panic!("actual target retained");
            };
            assert_eq!(host.calls, 1);
            assert!(
                _owner.staged.live_test_retired(),
                "callback refusal remains distinct from cancel"
            );
            assert!(_owner.staged.observation().is_none());
            assert_eq!(_owner.accounting.calls(), 1);
            assert_eq!(_owner.accounting.fuel(), 1);
            assert_eq!(_owner.accounting.request_bytes(), host.request.len() as u64);
            assert_eq!(
                _owner.accounting,
                _owner.staged.dispatch().unwrap().evidence().accounting()
            );
            if mode == 2 {
                assert_eq!(*error, SourceJournalError::Poisoned);
                assert_eq!(_owner.staged.failure(),Some(crate::interpreter::resumable::owned_frame::registered_stage::effect::OwnedEffectFailureV8::Target(Settlement::HostPanicked)));
                assert_eq!(_owner.accounting.result_bytes(), 0);
            } else {
                assert_eq!(*error, SourceJournalError::Binding);
                assert_eq!(_owner.staged.failure(),Some(crate::interpreter::resumable::owned_frame::registered_stage::effect::OwnedEffectFailureV8::Cancelled));
                assert_eq!(_owner.accounting.result_bytes(), host.result.len() as u64);
            }
            clock.post.set(0);
            assert_eq!(_owner.validate_live(), Err(SourceJournalError::Poisoned));
            assert!(journal.hold().is_err());
            assert_eq!(
                host.calls, 1,
                "later healthy clock cannot redispatch or release"
            );
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
