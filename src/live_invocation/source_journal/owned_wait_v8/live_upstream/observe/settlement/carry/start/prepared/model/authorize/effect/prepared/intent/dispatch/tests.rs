//! Genuine second target entry; immutable previous proof and live totals differ.
use super::super::tests::{prepared, test_staged};
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    Settlement, TargetEvidence, TargetHostError, TargetHostRequest, TargetResponseSink,
    TypedCarrier,
};
use crate::live_invocation::source_journal::SourceEffectFailure;
use std::cell::Cell;
pub(super) struct Host {
    pub calls: usize,
    pub mode: u8,
    pub request: Vec<u8>,
}
impl TargetHostHandler for Host {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        assert_eq!(request.turn(), 1);
        assert_eq!(request.fuel(), 1);
        self.request = request.canonical_wire();
        HOST.with(|v| v.set(true));
        match self.mode {
            1 => Err(TargetHostError::Failed),
            2 => sink
                .write(&vec![0; 65537])
                .map_err(|_| TargetHostError::Failed),
            3 => panic!("actual second host"),
            _ => {
                let payload=b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n".to_vec();
                let wire = TypedCarrier::new(request.operation().result_type(), payload)
                    .unwrap()
                    .encode();
                sink.write(&wire).map_err(|_| TargetHostError::Failed)
            }
        }
    }
}
pub(super) fn activated<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    staged: LiveContinuedAuthorizationV8<'j>,
) -> (
    LiveActivatedContinuedEffectV8<'j>,
    Vec<std::sync::Weak<[u8]>>,
) {
    let (owner, leaves) = prepared(journal, staged);
    let selected = owner
        .prepare_intent()
        .unwrap_or_else(|_| panic!("actual Intent"));
    let ack = journal
        .begin_session()
        .unwrap()
        .append_owned_continued_intent(selected)
        .unwrap_or_else(|_| panic!("physical Intent ACK"));
    (
        ack.advance_intent()
            .unwrap_or_else(|_| panic!("actual Activated")),
        leaves,
    )
}
pub(super) fn host(mode: u8) -> Host {
    HOST.with(|v| v.set(false));
    Host {
        calls: 0,
        mode,
        request: Vec::new(),
    }
}
thread_local! {static HOST:Cell<bool>=const{Cell::new(false)};static PANIC:Cell<bool>=const{Cell::new(false)};}
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        if HOST.with(Cell::get) && PANIC.with(Cell::get) {
            panic!("posthost clock")
        }
        1
    }
}
impl crate::live_invocation::SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_continued_dispatch_actual_second_host_preserves_positive_prior_and_exact_evidence() {
    test_staged(
        |journal, staged, _, prior| {
            let (actual, leaves) = activated(journal, staged);
            let mut host = host(0);
            let owner = actual
                .dispatch(&mut host)
                .unwrap_or_else(|_| panic!("actual dispatch"));
            assert_eq!(host.calls, 1);
            let (_, retired, exchange) = owner
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_dispatched()
                .unwrap();
            assert!(!retired);
            let (evidence, result) = exchange.unwrap();
            let evidence = TargetEvidence::decode(&evidence).unwrap();
            evidence
                .replay_exchange_wire(&host.request, result.as_deref())
                .unwrap();
            assert_eq!(evidence.settlement(), Settlement::Returned);
            assert_eq!(*owner.accounting(), evidence.accounting());
            assert_eq!(owner.accounting().calls(), prior.calls() + 1);
            assert_eq!(owner.accounting().fuel(), prior.fuel() + 1);
            assert_eq!(
                owner.accounting().request_bytes(),
                prior.request_bytes() + host.request.len() as u64
            );
            assert_eq!(
                owner.accounting().result_bytes(),
                prior.result_bytes() + result.unwrap().len() as u64
            );
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            drop(owner);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
#[test]
fn owned_continued_dispatch_cancel_before_entry_keeps_activated_and_prior_ledger() {
    test_staged(
        |journal, staged, _, prior| {
            let (actual, leaves) = activated(journal, staged);
            actual
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_dispatch_cancellation()
                .cancel();
            let mut host = host(0);
            let failed = actual.dispatch(&mut host).err().expect("cancelled");
            assert_eq!(host.calls, 0);
            let LiveContinuedDispatchFailureV8::Before { owner, error } = &failed else {
                panic!("original Activated")
            };
            assert_eq!(*error, SourceJournalError::Binding);
            assert_eq!(
                *owner
                    .phase
                    .owner
                    .owner
                    .authorization
                    .completed
                    .owner
                    .accounting(),
                prior
            );
            assert!(journal.hold().is_err());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
#[test]
fn owned_continued_dispatch_host_panic_stays_primary_after_clock_fault_and_retains_staged() {
    test_staged(
        |journal, staged, _, prior| {
            let (mut actual, leaves) = activated(journal, staged);
            let ModelOwnerV8::Resumed(owner) =
                &mut actual.phase.owner.owner.authorization.completed.owner
            else {
                panic!("resumed")
            };
            owner.owner.test_authorize_clock(&Clock);
            PANIC.with(|v| v.set(true));
            let mut host = host(3);
            let failed = actual.dispatch(&mut host).err().expect("posthost fault");
            assert_eq!(host.calls, 1);
            let LiveContinuedDispatchFailureV8::After { owner, error } = &failed else {
                panic!("actual Staged")
            };
            assert_eq!(*error, SourceJournalError::Poisoned);
            let (reason, retired, exchange) = owner
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_dispatched()
                .unwrap();
            assert_eq!(reason, Some(SourceEffectFailure::HandlerFailed));
            assert!(retired);
            let (evidence, _) = exchange.unwrap();
            let evidence = TargetEvidence::decode(&evidence).unwrap();
            assert_eq!(evidence.settlement(), Settlement::HostPanicked);
            assert_eq!(evidence.accounting(), *owner.accounting());
            assert_eq!(owner.accounting().calls(), prior.calls() + 1);
            assert_eq!(owner.validate_live(), Err(*error));
            assert!(journal.hold().is_err());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            PANIC.with(|v| v.set(false));
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}

#[test]
fn owned_continued_dispatch_physical_loss_and_cancel_retire_actual_staged_without_reentry() {
    test_staged(
        |journal, staged, _, prior| {
            let (actual, leaves) = activated(journal, staged);
            let cancel = actual
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_dispatch_cancellation();
            struct FaultHost<'a> {
                journal: &'a SourceOwnedWaitJournalV8,
                cancel: crate::agent_runtime::AgentCancellation,
                calls: usize,
            }
            impl TargetHostHandler for FaultHost<'_> {
                fn dispatch(
                    &mut self,
                    _: &TargetHostRequest,
                    _: &mut TargetResponseSink,
                ) -> Result<(), TargetHostError> {
                    self.calls += 1;
                    self.journal
                        .test_observe_lease()
                        .borrow_mut()
                        .append(b"x")
                        .unwrap();
                    self.cancel.cancel();
                    panic!("host failure stays primary")
                }
            }
            let mut host = FaultHost {
                journal,
                cancel,
                calls: 0,
            };
            let failure = actual.dispatch(&mut host).err().expect("physical fault");
            let LiveContinuedDispatchFailureV8::After { owner, error } = &failure else {
                panic!("actual Staged")
            };
            let (reason, retired, exchange) = owner
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_dispatched()
                .unwrap();
            assert!(retired);
            assert_eq!(reason, Some(SourceEffectFailure::HandlerFailed));
            assert_eq!(
                TargetEvidence::decode(&exchange.unwrap().0)
                    .unwrap()
                    .settlement(),
                Settlement::HostPanicked
            );
            assert_eq!(owner.accounting().calls(), prior.calls() + 1);
            assert_eq!(owner.validate_live(), Err(*error));
            assert_eq!(host.calls, 1);
            assert!(journal.hold().is_err());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            drop(failure);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
