use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use semaprax::codegen::emit_native_callable_v3_descriptor;
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;
use sha2::{Digest, Sha256};

use super::*;
use crate::callable_wire_v3::{
    action_chain_digest, arm_reusable_storage_allocation_failure, decision_digest, frame_digest,
    request_digest, response_storage_digest, ActionBoundary, CandidateOutcome, CellState, Decision,
    Disposition, DispositionCell, ExecuteOutcome, ExecuteReturn, FramePhase, ResourceCell,
};
use crate::descriptor_v3::{Action as GraphAction, Outcome as GraphOutcome, ResourceState};

#[derive(Clone)]
struct TestPin {
    instance: NonZeroU64,
    drops: Rc<RefCell<Vec<&'static str>>>,
    label: &'static str,
}

impl SettlementPin for TestPin {
    fn retain(&self) -> Self {
        Self {
            instance: self.instance,
            drops: Rc::clone(&self.drops),
            label: "retain",
        }
    }

    fn instance_nonce(&self) -> NonZeroU64 {
        self.instance
    }

    fn is_same_instance(&self, other: &Self) -> bool {
        self.instance == other.instance && Rc::ptr_eq(&self.drops, &other.drops)
    }
}

impl Drop for TestPin {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.label);
    }
}

fn descriptor() -> Descriptor {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let artifact = emit_native_callable_v3_descriptor(
        &corpus.program,
        &DeclarationId::new("token.discard-two"),
    )
    .unwrap();
    Descriptor::parse(artifact.bytes()).unwrap()
}

fn authority() -> ReceiptAuthority {
    ReceiptAuthority::from_os(NonZeroU64::new(91).unwrap()).unwrap()
}

fn ledger() -> (SettlementLedger<TestPin>, Rc<RefCell<Vec<&'static str>>>) {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let pin = TestPin {
        instance: NonZeroU64::new(91).unwrap(),
        drops: Rc::clone(&drops),
        label: "root",
    };
    (
        SettlementLedger::try_new(pin, descriptor(), authority()).unwrap(),
        drops,
    )
}

fn ledger_for(function: &str) -> (SettlementLedger<TestPin>, Rc<RefCell<Vec<&'static str>>>) {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let pin = TestPin {
        instance: NonZeroU64::new(91).unwrap(),
        drops: Rc::clone(&drops),
        label: "root",
    };
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let artifact =
        emit_native_callable_v3_descriptor(&corpus.program, &DeclarationId::new(function)).unwrap();
    let descriptor = Descriptor::parse(artifact.bytes()).unwrap();
    (
        SettlementLedger::try_new(pin, descriptor, authority()).unwrap(),
        drops,
    )
}

fn request(identity: RecoveryIdentity) -> ExecuteRequest {
    ExecuteRequest {
        identity: identity.call,
        arguments: vec![
            crate::callable_wire_v3::RequestArgument::Owned {
                index: 0,
                owner_ordinal: 0,
                payload: 10,
            },
            crate::callable_wire_v3::RequestArgument::Owned {
                index: 1,
                owner_ordinal: 1,
                payload: 20,
            },
        ],
    }
}

fn request_one(identity: RecoveryIdentity, payload: u64) -> ExecuteRequest {
    ExecuteRequest {
        identity: identity.call,
        arguments: vec![crate::callable_wire_v3::RequestArgument::Owned {
            index: 0,
            owner_ordinal: 0,
            payload,
        }],
    }
}

fn owners(ledger: &SettlementLedger<TestPin>) -> [SettlementOwnerHandle; 2] {
    [
        ledger.register_owner(4, 7).unwrap(),
        ledger.register_owner(5, 9).unwrap(),
    ]
}

#[test]
fn pair_registration_is_atomic_for_hostile_second_owner() {
    let (ledger, _drops) = ledger();
    let before = ledger.core.borrow().authoritative_owners.clone();
    assert_eq!(
        ledger.register_owner_pair([(4, 7), (4, 9)]),
        Err(SettlementLedgerError::DuplicateOwner)
    );
    assert_eq!(ledger.core.borrow().authoritative_owners, before);
    let handles = ledger.register_owner_pair([(4, 7), (5, 9)]).unwrap();
    assert_eq!(handles[0].slot, 4);
    assert_eq!(handles[1].slot, 5);
}

struct AbortEvidence {
    request: ExecuteRequest,
    response_storage: Vec<u8>,
    frame: RecoveryFrame,
    decision: SettlementDecision,
    actions: Vec<ActionRecord>,
    candidate: CandidateReceipt,
}

fn abort_evidence(descriptor: &Descriptor, identity: RecoveryIdentity) -> AbortEvidence {
    let request = ExecuteRequest {
        identity: identity.call,
        arguments: vec![
            crate::callable_wire_v3::RequestArgument::Owned {
                index: 0,
                owner_ordinal: 0,
                payload: 10,
            },
            crate::callable_wire_v3::RequestArgument::Owned {
                index: 1,
                owner_ordinal: 1,
                payload: 20,
            },
        ],
    };
    let request_hash = request_digest(&request.encode());
    let response_storage = vec![0; descriptor.capacities.execute_response as usize];
    let response_hash = response_storage_digest(9, &response_storage);
    let decision = SettlementDecision {
        identity,
        decision: Decision::AbortPhysical(9),
    };
    let decision_hash = decision_digest(&decision.encode());
    let checkpoint = descriptor.graph.checkpoints.first().unwrap();
    let payloads = [10_u64, 20_u64];
    let mut cells = checkpoint
        .resources
        .iter()
        .enumerate()
        .map(|(owner, state)| ResourceCell {
            state: match state {
                ResourceState::Live => CellState::Live,
                ResourceState::ProvisionalResult => CellState::ProvisionalResult,
                ResourceState::Dead => CellState::Dead,
                ResourceState::Finalizing | ResourceState::Published => unreachable!(),
            },
            payload: payloads[owner],
        })
        .collect::<Vec<_>>();
    let mut actions = Vec::new();
    for (semantic_index, owner) in checkpoint.abort_order.iter().copied().enumerate() {
        let before = cells[owner as usize];
        actions.push(ActionRecord {
            identity,
            semantic_action_index: semantic_index as u32,
            boundary: ActionBoundary::Started,
            owner_ordinal: owner,
            payload: before.payload,
            before: before.state,
            after: CellState::Finalizing,
            checkpoint: checkpoint.id,
        });
        actions.push(ActionRecord {
            identity,
            semantic_action_index: semantic_index as u32,
            boundary: ActionBoundary::Completed,
            owner_ordinal: owner,
            payload: before.payload,
            before: CellState::Finalizing,
            after: CellState::Dead,
            checkpoint: checkpoint.id,
        });
        cells[owner as usize].state = CellState::Dead;
    }
    let action_hash =
        action_chain_digest(decision_hash, checkpoint.abort_order.len(), &actions).unwrap();
    let mut frame = RecoveryFrame {
        identity,
        request_digest: request_hash,
        response_storage_digest: response_hash,
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::Returned(9),
        checkpoint: checkpoint.id,
        phase: FramePhase::ProviderSettled,
        decision_digest: decision_hash,
        next_action: checkpoint.abort_order.len() as u32,
        record_count: actions.len() as u32,
        active_finalizers: 0,
        cells,
        action_chain_digest: action_hash,
        pre_candidate_digest: [0; 32],
    };
    let provisional = frame.encode();
    frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
    let candidate = CandidateReceipt {
        identity,
        request_digest: request_hash,
        response_storage_digest: response_hash,
        semantic_trace_digest: [0; 32],
        frame_digest: frame.pre_candidate_digest,
        decision_digest: decision_hash,
        action_evidence_digest: action_hash,
        outcome: CandidateOutcome::Abort,
        active_finalizers: 0,
        dispositions: frame
            .cells
            .iter()
            .map(|cell| DispositionCell {
                disposition: Disposition::Dead,
                payload: cell.payload,
            })
            .collect(),
    };
    AbortEvidence {
        request,
        response_storage,
        frame,
        decision,
        actions,
        candidate,
    }
}

struct OwnedEvidence {
    request: ExecuteRequest,
    response_storage: Vec<u8>,
    response: ExecuteResponse,
    frame: RecoveryFrame,
    decision: SettlementDecision,
    actions: Vec<ActionRecord>,
    candidate: CandidateReceipt,
}

fn owned_evidence(descriptor: &Descriptor, identity: RecoveryIdentity) -> OwnedEvidence {
    let request = ExecuteRequest {
        identity: identity.call,
        arguments: vec![crate::callable_wire_v3::RequestArgument::Owned {
            index: 0,
            owner_ordinal: 0,
            payload: 42,
        }],
    };
    let checkpoint = descriptor
        .graph
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.outcome == Some(GraphOutcome::OwnedSuccess(0)))
        .unwrap();
    let trace = descriptor
        .graph
        .edges
        .iter()
        .find_map(|edge| match &edge.action {
            GraphAction::CertifyOutcome(evidence) if edge.to == checkpoint.id => Some(evidence),
            _ => None,
        })
        .unwrap();
    let response = ExecuteResponse {
        identity: identity.call,
        request_digest: request_digest(&request.encode()),
        checkpoint: checkpoint.id,
        outcome: ExecuteOutcome::Owned {
            owner_ordinal: 0,
            payload: 42,
        },
        event_ordinals: trace.ordinals.clone(),
        storage_capacity: descriptor.capacities.execute_response as usize,
    };
    let decision = SettlementDecision {
        identity,
        decision: Decision::AcceptOwned(0),
    };
    let decision_hash = decision_digest(&decision.encode());
    let payloads = request
        .arguments
        .iter()
        .filter_map(|argument| match argument {
            crate::callable_wire_v3::RequestArgument::Owned {
                owner_ordinal,
                payload,
                ..
            } => Some((*owner_ordinal, *payload)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut cells = checkpoint
        .resources
        .iter()
        .enumerate()
        .map(|(owner, state)| ResourceCell {
            state: match state {
                ResourceState::Live => CellState::Live,
                ResourceState::ProvisionalResult => CellState::ProvisionalResult,
                ResourceState::Dead => CellState::Dead,
                ResourceState::Finalizing | ResourceState::Published => unreachable!(),
            },
            payload: payloads[&(owner as u32)],
        })
        .collect::<Vec<_>>();
    let mut actions = Vec::new();
    let mut semantic_index = 0_u32;
    for owner in &checkpoint.accept_order {
        let cell = cells[*owner as usize];
        actions.push(ActionRecord {
            identity,
            semantic_action_index: semantic_index,
            boundary: ActionBoundary::Started,
            owner_ordinal: *owner,
            payload: cell.payload,
            before: cell.state,
            after: CellState::Finalizing,
            checkpoint: checkpoint.id,
        });
        actions.push(ActionRecord {
            identity,
            semantic_action_index: semantic_index,
            boundary: ActionBoundary::Completed,
            owner_ordinal: *owner,
            payload: cell.payload,
            before: CellState::Finalizing,
            after: CellState::Dead,
            checkpoint: checkpoint.id,
        });
        cells[*owner as usize].state = CellState::Dead;
        semantic_index += 1;
    }
    let cell = cells[0];
    actions.push(ActionRecord {
        identity,
        semantic_action_index: semantic_index,
        boundary: ActionBoundary::Publish,
        owner_ordinal: 0,
        payload: cell.payload,
        before: CellState::ProvisionalResult,
        after: CellState::Published,
        checkpoint: checkpoint.id,
    });
    cells[0].state = CellState::Published;
    semantic_index += 1;
    let action_hash =
        action_chain_digest(decision_hash, semantic_index as usize, &actions).unwrap();
    let response_storage = response.encode();
    let mut trace_hasher = Sha256::new();
    trace_hasher.update(b"semaprax.native-recovery-trace-evidence.v1\0");
    trace_hasher.update(descriptor.fingerprints.trace_path_certificate);
    trace_hasher.update((response.event_ordinals.len() as u64).to_le_bytes());
    for ordinal in &response.event_ordinals {
        trace_hasher.update(ordinal.to_le_bytes());
    }
    trace_hasher.update([2]);
    let semantic_trace_digest: [u8; 32] = trace_hasher.finalize().into();
    let mut frame = RecoveryFrame {
        identity,
        request_digest: request_digest(&request.encode()),
        response_storage_digest: response_storage_digest(0, &response_storage),
        semantic_trace_digest,
        execute_return: ExecuteReturn::Returned(0),
        checkpoint: checkpoint.id,
        phase: FramePhase::ProviderSettled,
        decision_digest: decision_hash,
        next_action: semantic_index,
        record_count: actions.len() as u32,
        active_finalizers: 0,
        cells,
        action_chain_digest: action_hash,
        pre_candidate_digest: [0; 32],
    };
    let provisional = frame.encode();
    frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
    let candidate = CandidateReceipt {
        identity,
        request_digest: frame.request_digest,
        response_storage_digest: frame.response_storage_digest,
        semantic_trace_digest,
        frame_digest: frame.pre_candidate_digest,
        decision_digest: decision_hash,
        action_evidence_digest: action_hash,
        outcome: CandidateOutcome::Owned(0),
        active_finalizers: 0,
        dispositions: vec![DispositionCell {
            disposition: Disposition::Published,
            payload: 42,
        }],
    };
    OwnedEvidence {
        request,
        response_storage,
        response,
        frame,
        decision,
        actions,
        candidate,
    }
}

#[test]
fn reservation_is_monotonic_exact_instance_and_fully_preallocated() {
    let (ledger, _) = ledger();
    let first = ledger.reserve().unwrap();
    let second = ledger.reserve().unwrap();
    assert_ne!(
        first.identity().call.invocation,
        second.identity().call.invocation
    );
    assert_ne!(
        first.identity().call.frame_generation,
        second.identity().call.frame_generation
    );
    assert_ne!(
        first.identity().call.provider_challenge,
        second.identity().call.provider_challenge
    );
    assert_eq!(
        first.frame.as_ref().unwrap().storage.request.len(),
        ledger.core.borrow().descriptor.capacities.request as usize
    );
    assert_eq!(
        first.frame.as_ref().unwrap().storage.receipt.len(),
        HOST_RECEIPT_BYTES
    );
    assert_eq!(
        first
            .frame
            .as_ref()
            .unwrap()
            .storage
            .before_entries
            .capacity(),
        2
    );
}

#[test]
fn authority_cannot_be_reused_for_another_exact_instance() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let pin = TestPin {
        instance: NonZeroU64::new(91).unwrap(),
        drops,
        label: "root",
    };
    let wrong_authority = ReceiptAuthority::from_os(NonZeroU64::new(92).unwrap()).unwrap();
    assert!(matches!(
        SettlementLedger::try_new(pin, descriptor(), wrong_authority),
        Err(SettlementLedgerError::WrongInstance)
    ));
}

#[test]
fn exhausted_counter_does_not_consume_preallocated_storage_or_advance_peer_counter() {
    let (ledger, _) = ledger();
    ledger.core.borrow_mut().next_generation = u64::MAX;
    let active_before = ledger.core.borrow().active_storage.len();
    let invocation_before = ledger.core.borrow().next_invocation;
    assert_eq!(
        ledger.reserve().err(),
        Some(SettlementLedgerError::CounterExhausted)
    );
    assert_eq!(ledger.core.borrow().active_storage.len(), active_before);
    assert_eq!(ledger.core.borrow().next_invocation, invocation_before);
}

#[test]
fn call_commit_and_finalizer_boundaries_are_fail_closed() {
    let (ledger, _) = ledger();
    let handles = owners(&ledger);
    let mut frame = ledger.reserve().unwrap();
    assert_eq!(frame.call_commit(), Err(SettlementLedgerError::NotStaged));
    let request = request(frame.identity());
    frame.stage_call(&request, &handles).unwrap();
    frame.call_commit().unwrap();
    assert_eq!(frame.call_commit(), Err(SettlementLedgerError::WrongPhase));
    let decision = SettlementDecision {
        identity: frame.identity(),
        decision: crate::callable_wire_v3::Decision::AbortHostUnwind,
    };
    frame.decision_commit(&decision).unwrap();
    frame.finalizer_started().unwrap();
    assert_eq!(
        frame.provider_settled(),
        Err(SettlementLedgerError::WrongPhase)
    );
    drop(frame);
    assert!(ledger.is_poisoned());
    assert!(ledger.is_draining());
    assert_eq!(ledger.quarantined_count(), 1);
    assert_eq!(
        ledger.reserve().err(),
        Some(SettlementLedgerError::Poisoned)
    );
}

#[test]
fn duplicate_stale_and_cross_frame_owner_use_reject_before_mutation() {
    let (ledger, _) = ledger();
    let handles = owners(&ledger);

    let mut duplicate = ledger.reserve().unwrap();
    let duplicate_request = request(duplicate.identity());
    duplicate
        .stage_call(&duplicate_request, &[handles[0], handles[0]])
        .unwrap();
    assert_eq!(
        duplicate.call_commit(),
        Err(SettlementLedgerError::DuplicateOwner)
    );
    drop(duplicate);
    assert!(!ledger.is_poisoned());

    let mut stale_handle = handles[0];
    stale_handle.generation += 1;
    let mut stale = ledger.reserve().unwrap();
    let stale_request = request(stale.identity());
    stale
        .stage_call(&stale_request, &[stale_handle, handles[1]])
        .unwrap();
    assert_eq!(stale.call_commit(), Err(SettlementLedgerError::StaleOwner));
    drop(stale);
    assert!(!ledger.is_poisoned());

    let mut first = ledger.reserve().unwrap();
    let mut second = ledger.reserve().unwrap();
    let first_request = request(first.identity());
    let second_request = request(second.identity());
    first.stage_call(&first_request, &handles).unwrap();
    second.stage_call(&second_request, &handles).unwrap();
    first.call_commit().unwrap();
    assert_eq!(second.call_commit(), Err(SettlementLedgerError::StaleOwner));
    drop(second);
    assert!(!ledger.is_poisoned());
    drop(first);
    assert!(ledger.is_poisoned());
    assert_eq!(ledger.quarantined_count(), 1);
}

#[test]
fn raii_drop_and_unwind_absorb_every_postcommit_phase() {
    for phase in 0..5 {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let mut transaction = ledger.reserve().unwrap();
        let request = request(transaction.identity());
        transaction.stage_call(&request, &handles).unwrap();
        transaction.call_commit().unwrap();
        let decision = SettlementDecision {
            identity: transaction.identity(),
            decision: Decision::AbortHostUnwind,
        };
        if phase >= 1 {
            transaction.decision_commit(&decision).unwrap();
        }
        if phase == 2 {
            transaction.finalizer_started().unwrap();
        }
        if phase >= 3 {
            transaction.finalizer_started().unwrap();
            transaction.finalizer_completed().unwrap();
        }
        if phase == 4 {
            transaction.provider_settled().unwrap();
        }
        drop(transaction);
        assert!(ledger.is_poisoned(), "phase {phase} did not poison");
        assert_eq!(ledger.quarantined_count(), 1);
    }

    let (ledger, _) = ledger();
    let handles = owners(&ledger);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut transaction = ledger.reserve().unwrap();
        let request = request(transaction.identity());
        transaction.stage_call(&request, &handles).unwrap();
        transaction.call_commit().unwrap();
        panic!("injected postcommit unwind");
    }));
    assert!(unwind.is_err());
    assert!(ledger.is_poisoned());
    assert_eq!(ledger.quarantined_count(), 1);
}

#[test]
fn precommit_drop_recycles_and_quarantine_permits_bound_all_outstanding_frames() {
    let (ledger, _) = ledger();
    {
        let mut outstanding = Vec::new();
        for _ in 0..64 {
            outstanding.push(ledger.reserve().unwrap());
        }
        assert_eq!(
            ledger.reserve().err(),
            Some(SettlementLedgerError::CapacityExhausted)
        );
    }
    assert!(!ledger.is_poisoned());
    assert_eq!(ledger.core.borrow().available_quarantine.len(), 64);
    assert!(ledger.reserve().is_ok());
}

#[test]
fn forgotten_postcommit_guard_leaves_future_reservation_fail_closed() {
    let (ledger, _) = ledger();
    let handles = owners(&ledger);
    let mut transaction = ledger.reserve().unwrap();
    let request = request(transaction.identity());
    transaction.stage_call(&request, &handles).unwrap();
    transaction.call_commit().unwrap();
    std::mem::forget(transaction);
    assert_eq!(ledger.core.borrow().active_postcommit, 1);
    assert_eq!(
        ledger.reserve().err(),
        Some(SettlementLedgerError::Draining)
    );
}

#[test]
fn call_commit_preserves_every_preallocated_buffer_and_capacity() {
    let (ledger, _) = ledger();
    let handles = owners(&ledger);
    let mut frame = ledger.reserve().unwrap();
    let request = request(frame.identity());
    frame.stage_call(&request, &handles).unwrap();
    let before = frame.buffer_signature();
    frame.call_commit().unwrap();
    assert_eq!(before, frame.buffer_signature());
}

#[test]
fn leaf_pins_drop_after_quarantine_evidence_and_root_drops_last() {
    let (ledger, drops) = ledger();
    let handles = owners(&ledger);
    let mut frame = ledger.reserve().unwrap();
    let request = request(frame.identity());
    frame.stage_call(&request, &handles).unwrap();
    frame.call_commit().unwrap();
    let decision = SettlementDecision {
        identity: frame.identity(),
        decision: crate::callable_wire_v3::Decision::AbortHostUnwind,
    };
    frame.decision_commit(&decision).unwrap();
    frame.finalizer_started().unwrap();
    drop(frame);
    assert!(drops.borrow().is_empty());
    drop(ledger);
    assert_eq!(drops.borrow().as_slice(), ["retain", "root"]);
}

#[test]
fn receipt_commit_is_atomic_replay_is_exact_and_conflict_preserves_original() {
    let (ledger, _) = ledger();
    let handles = owners(&ledger);
    let mut reserved = ledger.reserve().unwrap();
    let evidence = abort_evidence(&ledger.core.borrow().descriptor, reserved.identity());
    reserved.stage_call(&evidence.request, &handles).unwrap();
    reserved.call_commit().unwrap();
    reserved.decision_commit(&evidence.decision).unwrap();
    reserved
        .candidate_storage_mut()
        .copy_from_slice(&evidence.candidate.encode());
    reserved.provider_settled().unwrap();
    let identity = reserved.identity();
    let candidate_bytes = evidence.candidate.encode();
    let result = reserved
        .receipt_commit(ReceiptCommitEvidence {
            request: &evidence.request,
            execute_return_code: 9,
            response_storage: ResponseStorageEvidence::External(&evidence.response_storage),
            response: None,
            frame: &evidence.frame,
            decision: &evidence.decision,
            actions: &evidence.actions,
            candidate: &evidence.candidate,
        })
        .unwrap();
    assert_eq!(result.publication, Publication::NoOwned);
    assert_ne!(result.ledger_before, result.ledger_after);
    let owners_after_commit = ledger.core.borrow().authoritative_owners.clone();
    assert_eq!(
        ledger.replay_committed(identity, &candidate_bytes).unwrap(),
        result
    );
    assert_eq!(
        ledger.core.borrow().authoritative_owners,
        owners_after_commit
    );
    let original = ledger.committed_result(identity).unwrap();
    let mut conflicting = candidate_bytes;
    *conflicting.last_mut().unwrap() ^= 1;
    assert_eq!(
        ledger.replay_committed(identity, &conflicting),
        Err(SettlementLedgerError::ConflictingReplay)
    );
    assert_eq!(ledger.committed_result(identity), Some(original));
    assert_eq!(
        ledger.core.borrow().authoritative_owners,
        owners_after_commit
    );
    assert!(ledger.is_poisoned());
    assert!(ledger.is_draining());
    assert_eq!(ledger.quarantined_count(), 1);
}

#[test]
fn owned_commit_returns_one_refreshed_handle_old_is_stale_and_replay_is_idempotent() {
    let (ledger, _) = ledger_for("token.identity");
    let old = ledger.register_owner(4, 7).unwrap();
    let mut transaction = ledger.reserve().unwrap();
    let evidence = owned_evidence(&ledger.core.borrow().descriptor, transaction.identity());
    transaction.stage_call(&evidence.request, &[old]).unwrap();
    transaction.call_commit().unwrap();
    transaction.decision_commit(&evidence.decision).unwrap();
    transaction
        .candidate_storage_mut()
        .copy_from_slice(&evidence.candidate.encode());
    transaction.provider_settled().unwrap();
    let identity = transaction.identity();
    let candidate_bytes = evidence.candidate.encode();
    let committed = transaction
        .receipt_commit(ReceiptCommitEvidence {
            request: &evidence.request,
            execute_return_code: 0,
            response_storage: ResponseStorageEvidence::External(&evidence.response_storage),
            response: Some(&evidence.response),
            frame: &evidence.frame,
            decision: &evidence.decision,
            actions: &evidence.actions,
            candidate: &evidence.candidate,
        })
        .unwrap();
    let refreshed = committed.published_owner.unwrap();
    assert_eq!(refreshed.slot, old.slot);
    assert_eq!(refreshed.generation, old.generation + 1);
    let owners_after_commit = ledger.core.borrow().authoritative_owners.clone();
    assert_eq!(
        ledger.replay_committed(identity, &candidate_bytes).unwrap(),
        committed
    );
    assert_eq!(
        ledger.core.borrow().authoritative_owners,
        owners_after_commit
    );

    let mut stale = ledger.reserve().unwrap();
    let stale_request = request_one(stale.identity(), 42);
    stale.stage_call(&stale_request, &[old]).unwrap();
    assert_eq!(stale.call_commit(), Err(SettlementLedgerError::StaleOwner));
    drop(stale);

    let mut reusable = ledger.reserve().unwrap();
    let reusable_request = request_one(reusable.identity(), 42);
    reusable
        .stage_call(&reusable_request, &[refreshed])
        .unwrap();
    reusable.call_commit().unwrap();
    drop(reusable);
    assert!(ledger.is_poisoned());
}

#[test]
fn receipt_hmac_panic_quarantines_exact_evidence_and_retains_leaf_pin() {
    let (ledger, drops) = ledger_for("token.identity");
    let owner = ledger.register_owner(4, 7).unwrap();
    let mut transaction = ledger.reserve().unwrap();
    let evidence = owned_evidence(&ledger.core.borrow().descriptor, transaction.identity());
    transaction.stage_call(&evidence.request, &[owner]).unwrap();
    transaction.call_commit().unwrap();
    transaction.decision_commit(&evidence.decision).unwrap();
    transaction
        .execute_response_storage_mut()
        .copy_from_slice(&evidence.response_storage);
    let frame_bytes = evidence.frame.encode();
    transaction
        .frame_storage_mut()
        .copy_from_slice(&frame_bytes);
    let action_bytes = evidence.actions[0].encode();
    transaction
        .action_storage_mut()
        .copy_from_slice(&action_bytes);
    let candidate_bytes = evidence.candidate.encode();
    transaction
        .candidate_storage_mut()
        .copy_from_slice(&candidate_bytes);
    transaction.provider_settled().unwrap();
    let identity = transaction.identity();
    let request_bytes = evidence.request.encode();

    arm_receipt_prepare_panic();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = transaction.receipt_commit(ReceiptCommitEvidence {
            request: &evidence.request,
            execute_return_code: 0,
            response_storage: ResponseStorageEvidence::Reserved,
            response: Some(&evidence.response),
            frame: &evidence.frame,
            decision: &evidence.decision,
            actions: &evidence.actions,
            candidate: &evidence.candidate,
        });
    }));
    assert!(panic.is_err());

    {
        let core = ledger.core.borrow();
        assert_eq!(core.active_postcommit, 0);
        assert!(core.poisoned);
        assert!(core.draining);
        assert_eq!(core.quarantined_count(), 1);
        let quarantined = core.quarantined.iter().flatten().next().unwrap();
        assert_eq!(quarantined.frame.identity, identity);
        assert_eq!(quarantined.frame.storage.request, request_bytes);
        assert_eq!(
            quarantined.frame.storage.execute_response,
            evidence.response_storage
        );
        assert_eq!(quarantined.frame.storage.frame, frame_bytes);
        assert_eq!(quarantined.frame.storage.action, action_bytes);
        assert_eq!(quarantined.frame.storage.candidate, candidate_bytes);
        assert!(matches!(
            core.authoritative_owners[0].state,
            AuthoritativeOwnerState::Quarantined
        ));
    }
    assert!(
        drops.borrow().is_empty(),
        "quarantine must retain the leaf pin"
    );
    drop(ledger);
    assert_eq!(drops.borrow().as_slice(), ["retain", "root"]);
}

#[test]
fn postcommit_decode_allocation_failure_quarantines_exact_reserved_evidence() {
    let (ledger, drops) = ledger_for("token.identity");
    let owner = ledger.register_owner(4, 7).unwrap();
    let mut transaction = ledger.reserve().unwrap();
    let evidence = owned_evidence(&ledger.core.borrow().descriptor, transaction.identity());
    transaction.stage_call(&evidence.request, &[owner]).unwrap();
    transaction.call_commit().unwrap();
    transaction
        .execute_response_storage_mut()
        .copy_from_slice(&evidence.response_storage);
    let frame_bytes = evidence.frame.encode();
    transaction
        .frame_storage_mut()
        .copy_from_slice(&frame_bytes);
    let candidate_bytes = evidence.candidate.encode();
    transaction
        .candidate_storage_mut()
        .copy_from_slice(&candidate_bytes);
    let identity = transaction.identity();
    let request_bytes = transaction.request_bytes().to_vec();

    arm_reusable_storage_allocation_failure();
    assert_eq!(
        ExecuteResponse::parse_reusing(
            transaction.execute_response_bytes(),
            &ledger.core.borrow().descriptor,
            Vec::new(),
        ),
        Err(WireError::CapacityMismatch)
    );
    drop(transaction);

    assert!(ledger.is_poisoned());
    assert!(ledger.is_draining());
    assert_eq!(ledger.quarantined_count(), 1);
    let core = ledger.core.borrow();
    assert_eq!(core.active_postcommit, 0);
    let quarantined = core.quarantined.iter().flatten().next().unwrap();
    assert_eq!(quarantined.frame.identity, identity);
    assert_eq!(quarantined.frame.storage.request, request_bytes);
    assert_eq!(
        quarantined.frame.storage.execute_response,
        evidence.response_storage
    );
    assert_eq!(quarantined.frame.storage.frame, frame_bytes);
    assert_eq!(quarantined.frame.storage.candidate, candidate_bytes);
    assert_eq!(
        core.authoritative_owners[0].state,
        AuthoritativeOwnerState::Quarantined
    );
    assert!(
        drops.borrow().is_empty(),
        "leaf pin must remain quarantined"
    );
}

#[test]
fn failed_postcommit_evidence_is_absorbed_and_never_retried() {
    let (ledger, _) = ledger();
    let handles = owners(&ledger);
    let mut reserved = ledger.reserve().unwrap();
    let mut evidence = abort_evidence(&ledger.core.borrow().descriptor, reserved.identity());
    reserved.stage_call(&evidence.request, &handles).unwrap();
    reserved.call_commit().unwrap();
    reserved.decision_commit(&evidence.decision).unwrap();
    reserved.provider_settled().unwrap();
    evidence.candidate.frame_digest[0] ^= 1;
    assert!(matches!(
        reserved.receipt_commit(ReceiptCommitEvidence {
            request: &evidence.request,
            execute_return_code: 9,
            response_storage: ResponseStorageEvidence::External(&evidence.response_storage),
            response: None,
            frame: &evidence.frame,
            decision: &evidence.decision,
            actions: &evidence.actions,
            candidate: &evidence.candidate,
        }),
        Err(SettlementLedgerError::Wire(_))
    ));
    assert!(ledger.is_poisoned());
    assert!(ledger.is_draining());
    assert_eq!(ledger.quarantined_count(), 1);
    assert_eq!(
        ledger.reserve().err(),
        Some(SettlementLedgerError::Poisoned)
    );
}
