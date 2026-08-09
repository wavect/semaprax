//! Private composition root for an already-admitted callable-v3 image.
//!
//! This is intentionally not re-exported and performs no library admission.
//! It only connects the loader's exact leaf pin to the host receipt/ledger
//! authority after both independent descriptor and loader gates succeeded.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable-v3 public admission remains closed by SPX-B104"
)]

#[cfg(not(target_os = "ios"))]
use semaprax_native_loader::NativeSettlementModuleLease;
use semaprax_native_loader::{
    IosStaticTarget, ModuleInstanceId, NativeStaticSettlementLease, PreparedSettlementCall,
    PreparedSettlementExecute, SettlementBufferCapacities, SettlementCallError,
};

use crate::callable_wire_v3::{
    frame_digest, validate_successful_execute_evidence_preencoded, ActionBoundary, ActionRecord,
    CandidateOutcome, CandidateReceipt, CellState, Decision, ExecuteOutcome, ExecuteRequest,
    ExecuteResponse, ExecuteReturn, FramePhase, RecoveryFrame, RecoveryIdentity, RequestArgument,
    ResourceCell, SettlementDecision, WireError, PRE_EXECUTE_HOST_UNWIND_CODE,
};
use crate::descriptor_v3::{Descriptor, Linkage, Parameter, ResourceState, ScalarKind};
use crate::receipt_authority::ReceiptAuthority;
use crate::settlement_ledger::{
    CommittedResult, ReceiptCommitEvidence, ResponseStorageEvidence, SettlementLedger,
    SettlementLedgerError, SettlementOwnerHandle, SettlementPin, SettlementTransaction,
};

/// One private exact callable-v3 instance, independent of how code was bound.
///
/// Dynamic and iOS-static admission intentionally converge before the ledger:
/// generations, draining, frame pins, settlement and quarantine therefore use
/// one authoritative implementation.
pub(crate) enum SettlementInstanceLease {
    #[cfg(not(target_os = "ios"))]
    Dynamic(NativeSettlementModuleLease),
    IosStatic(NativeStaticSettlementLease),
}

impl SettlementInstanceLease {
    fn retain(&self) -> Self {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => Self::Dynamic(lease.retain()),
            Self::IosStatic(lease) => Self::IosStatic(lease.retain()),
        }
    }

    fn instance_id(&self) -> ModuleInstanceId {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => lease.instance_id(),
            Self::IosStatic(lease) => lease.instance_id(),
        }
    }

    fn descriptor_matches(&self, candidate: &[u8]) -> bool {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => lease.descriptor_matches(candidate),
            Self::IosStatic(lease) => lease.descriptor_matches(candidate),
        }
    }

    fn capacities(&self) -> SettlementBufferCapacities {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => lease.capacities(),
            Self::IosStatic(lease) => lease.capacities(),
        }
    }

    fn prepare_execute(&self) -> Result<PreparedSettlementExecute, SettlementCallError> {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => lease.prepare_execute(),
            Self::IosStatic(lease) => lease.prepare_execute(),
        }
    }

    fn invoke_execute(
        &self,
        call: &mut PreparedSettlementExecute,
    ) -> Result<u32, SettlementCallError> {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => lease.invoke_execute(call),
            Self::IosStatic(lease) => lease.invoke_execute(call),
        }
    }

    fn invoke_settle(&self, call: &mut PreparedSettlementCall) -> Result<u32, SettlementCallError> {
        match self {
            #[cfg(not(target_os = "ios"))]
            Self::Dynamic(lease) => lease.invoke_settle(call),
            Self::IosStatic(lease) => lease.invoke_settle(call),
        }
    }

    fn is_same_instance(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(not(target_os = "ios"))]
            (Self::Dynamic(left), Self::Dynamic(right)) => left.is_same_instance(right),
            (Self::IosStatic(left), Self::IosStatic(right)) => left.is_same_instance(right),
            #[cfg(not(target_os = "ios"))]
            (Self::Dynamic(_), Self::IosStatic(_)) | (Self::IosStatic(_), Self::Dynamic(_)) => {
                false
            }
        }
    }
}

impl SettlementPin for SettlementInstanceLease {
    fn retain(&self) -> Self {
        self.retain()
    }

    fn instance_nonce(&self) -> std::num::NonZeroU64 {
        std::num::NonZeroU64::new(self.instance_id().get())
            .expect("native instance identities are structurally nonzero")
    }

    fn is_same_instance(&self, other: &Self) -> bool {
        self.is_same_instance(other)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivateSettlementExecutionError {
    Ledger(SettlementLedgerError),
    Loader(SettlementCallError),
    Wire(WireError),
    ProviderSettlementRejected {
        code: u32,
        phase: FramePhase,
        active_finalizers: u32,
        next_action: u32,
        record_count: u32,
        frame_digest: [u8; 32],
    },
    UnsupportedFixture,
}

impl From<SettlementLedgerError> for PrivateSettlementExecutionError {
    fn from(value: SettlementLedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<SettlementCallError> for PrivateSettlementExecutionError {
    fn from(value: SettlementCallError) -> Self {
        Self::Loader(value)
    }
}

impl From<WireError> for PrivateSettlementExecutionError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivatePhysicalCommit {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) outcome: ExecuteOutcome,
    pub(crate) candidate_bytes: Vec<u8>,
    pub(crate) committed: CommittedResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateHostUnwindCommit {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) candidate_bytes: Vec<u8>,
    pub(crate) committed: CommittedResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateRecoveryCommit {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) outcome: Option<ExecuteOutcome>,
    pub(crate) decision: Decision,
    pub(crate) candidate_bytes: Vec<u8>,
    pub(crate) committed: CommittedResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateRecoveryPolicy {
    ValidateResponse,
    AbortMalformed,
}

/// Canonical private argument lane for the descriptor-v3 signature. Owned
/// handles carry ledger authority while their payload remains an exact wire
/// value; scalar values carry no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivateSettlementArgumentV3 {
    I64(i64),
    Bool(bool),
    Owned {
        handle: SettlementOwnerHandle,
        payload: u64,
    },
}

/// Private exact-instance runtime. The outer loader pin is last, while every
/// frame/cache/quarantine owns its own explicit retain through the ledger.
pub(crate) struct PrivateSettlementHostV3 {
    descriptor: Descriptor,
    ledger: SettlementLedger<SettlementInstanceLease>,
    module_lease: SettlementInstanceLease,
}

// This production-only assertion is intentionally compiled by every iOS
// target check. It proves that the exact static loader lease composes into the
// same private host ledger without creating a public admission constructor.
#[cfg(target_os = "ios")]
const _: fn(
    NativeStaticSettlementLease,
    &[u8],
) -> Result<PrivateSettlementHostV3, SettlementLedgerError> =
    PrivateSettlementHostV3::from_static_admitted;

impl PrivateSettlementHostV3 {
    #[cfg(not(target_os = "ios"))]
    pub(crate) fn from_admitted(
        module_lease: NativeSettlementModuleLease,
        expected_descriptor: &[u8],
    ) -> Result<Self, SettlementLedgerError> {
        Self::from_exact(
            SettlementInstanceLease::Dynamic(module_lease),
            expected_descriptor,
            None,
        )
    }

    pub(crate) fn from_static_admitted(
        module_lease: NativeStaticSettlementLease,
        expected_descriptor: &[u8],
    ) -> Result<Self, SettlementLedgerError> {
        let target = module_lease.target();
        Self::from_exact(
            SettlementInstanceLease::IosStatic(module_lease),
            expected_descriptor,
            Some(target),
        )
    }

    fn from_exact(
        module_lease: SettlementInstanceLease,
        expected_descriptor: &[u8],
        static_target: Option<IosStaticTarget>,
    ) -> Result<Self, SettlementLedgerError> {
        let descriptor = parse_exact_admitted_descriptor(
            expected_descriptor,
            |candidate| module_lease.descriptor_matches(candidate),
            static_target,
        )?;
        let loader = module_lease.capacities();
        if loader.request() != descriptor.capacities.request as usize
            || loader.execute_response() != descriptor.capacities.execute_response as usize
            || loader.frame() != descriptor.capacities.frame as usize
            || loader.decision() != descriptor.capacities.decision as usize
            || loader.candidate_receipt() != descriptor.capacities.candidate_receipt as usize
        {
            return Err(SettlementLedgerError::CapacityExhausted);
        }
        let instance_nonce = std::num::NonZeroU64::new(module_lease.instance_id().get())
            .expect("loader instance identities are structurally nonzero");
        let authority = ReceiptAuthority::from_os(instance_nonce)?;
        let ledger =
            SettlementLedger::try_new(module_lease.retain(), descriptor.clone(), authority)?;
        Ok(Self {
            descriptor,
            ledger,
            module_lease,
        })
    }

    pub(crate) fn register_owner(
        &self,
        slot: u64,
        generation: u64,
    ) -> Result<SettlementOwnerHandle, SettlementLedgerError> {
        self.ledger.register_owner(slot, generation)
    }

    /// Execute one canonical descriptor-v3 argument vector through an exact
    /// generated provider, loader lease and authoritative host ledger.
    ///
    /// The handoff is explicit: canonical host request/frame bytes seed the
    /// loader's disjoint one-shot buffers before `CallCommit`; provider output
    /// is copied back into host-owned evidence buffers before independent
    /// decoding. No provider-mutated shadow frame is treated as authority.
    pub(crate) fn execute_owned_success(
        &self,
        owners: &[SettlementOwnerHandle],
        payloads: &[u64],
    ) -> Result<PrivatePhysicalCommit, PrivateSettlementExecutionError> {
        if owners.len() != payloads.len() {
            return Err(PrivateSettlementExecutionError::UnsupportedFixture);
        }
        let mut arguments = Vec::with_capacity(self.descriptor.parameters.len());
        for parameter in &self.descriptor.parameters {
            let Parameter::Owned { owner_ordinal, .. } = parameter else {
                return Err(PrivateSettlementExecutionError::UnsupportedFixture);
            };
            let handle = *owners
                .get(*owner_ordinal)
                .ok_or(PrivateSettlementExecutionError::UnsupportedFixture)?;
            let payload = *payloads
                .get(*owner_ordinal)
                .ok_or(PrivateSettlementExecutionError::UnsupportedFixture)?;
            arguments.push(PrivateSettlementArgumentV3::Owned { handle, payload });
        }
        self.execute_canonical(&arguments)
    }

    pub(crate) fn execute_canonical(
        &self,
        arguments: &[PrivateSettlementArgumentV3],
    ) -> Result<PrivatePhysicalCommit, PrivateSettlementExecutionError> {
        let recovered = self.execute_canonical_recovery(arguments)?;
        let outcome = recovered
            .outcome
            .ok_or(PrivateSettlementExecutionError::UnsupportedFixture)?;
        Ok(PrivatePhysicalCommit {
            identity: recovered.identity,
            outcome,
            candidate_bytes: recovered.candidate_bytes,
            committed: recovered.committed,
        })
    }

    pub(crate) fn execute_canonical_recovery(
        &self,
        arguments: &[PrivateSettlementArgumentV3],
    ) -> Result<PrivateRecoveryCommit, PrivateSettlementExecutionError> {
        self.execute_canonical_recovery_inner(arguments, PrivateRecoveryPolicy::ValidateResponse)
    }

    #[cfg(test)]
    pub(crate) fn execute_canonical_abort_malformed(
        &self,
        arguments: &[PrivateSettlementArgumentV3],
    ) -> Result<PrivateRecoveryCommit, PrivateSettlementExecutionError> {
        self.execute_canonical_recovery_inner(arguments, PrivateRecoveryPolicy::AbortMalformed)
    }

    fn execute_canonical_recovery_inner(
        &self,
        arguments: &[PrivateSettlementArgumentV3],
        policy: PrivateRecoveryPolicy,
    ) -> Result<PrivateRecoveryCommit, PrivateSettlementExecutionError> {
        let mut transaction = self.reserve()?;
        let identity = transaction.identity();
        let (request, owners, payloads) = self.canonical_request(identity, arguments)?;
        transaction.stage_call(&request, &owners)?;

        let initial_frame = self.initial_frame(identity, &request, &payloads)?;
        let initial_frame_bytes = initial_frame.encode();
        let mut provider = self.module_lease.prepare_execute()?;
        let mut candidate_bytes = vec![0; self.descriptor.capacities.candidate_receipt as usize];
        let mut actions = Vec::with_capacity(
            self.descriptor
                .capacities
                .resource_count
                .saturating_mul(2)
                .saturating_add(1) as usize,
        );
        let mut replay_states =
            Vec::with_capacity(self.descriptor.capacities.resource_count as usize);
        let response_events = Vec::with_capacity(self.descriptor.capacities.event_count as usize);
        let executed_cells = Vec::with_capacity(self.descriptor.capacities.resource_count as usize);
        let settled_cells = Vec::with_capacity(self.descriptor.capacities.resource_count as usize);
        let candidate_dispositions =
            Vec::with_capacity(self.descriptor.capacities.resource_count as usize);
        if provider.request_storage().len() != transaction.request_bytes().len()
            || provider.frame_storage().len() != initial_frame_bytes.len()
        {
            return Err(PrivateSettlementExecutionError::UnsupportedFixture);
        }
        provider
            .request_storage_mut()
            .copy_from_slice(transaction.request_bytes());
        provider
            .frame_storage_mut()
            .copy_from_slice(&initial_frame_bytes);
        transaction
            .frame_storage_mut()
            .copy_from_slice(&initial_frame_bytes);

        // Every allocation and exact-capacity check needed by the provider's
        // physical buffers has completed before this irreversible boundary.
        #[cfg(any(
            test,
            all(feature = "unstable-android-emulator-harness", target_os = "android"),
            all(
                feature = "unstable-ios-simulator-harness",
                target_os = "ios",
                target_abi = "sim"
            )
        ))]
        let postcommit_allocation_probe = crate::postcommit_allocation_probe::begin();
        transaction.call_commit()?;
        let execute_return = self.module_lease.invoke_execute(&mut provider)?;
        transaction
            .execute_response_storage_mut()
            .copy_from_slice(provider.response_storage());
        transaction
            .frame_storage_mut()
            .copy_from_slice(provider.frame_storage());

        let executed_frame = RecoveryFrame::parse_reusing(
            transaction.frame_bytes(),
            &self.descriptor,
            executed_cells,
        )?;
        let (response, decision_kind) = if execute_return != 0 {
            (None, Decision::AbortPhysical(execute_return))
        } else if policy == PrivateRecoveryPolicy::AbortMalformed {
            (None, Decision::AbortMalformed)
        } else {
            match ExecuteResponse::parse_reusing(
                transaction.execute_response_bytes(),
                &self.descriptor,
                response_events,
            ) {
                Ok(response) => {
                    validate_successful_execute_evidence_preencoded(
                        &self.descriptor,
                        &request,
                        transaction.request_bytes(),
                        execute_return,
                        transaction.execute_response_bytes(),
                        &response,
                        &executed_frame,
                    )?;
                    let decision = match response.outcome {
                        ExecuteOutcome::Scalar { .. } => Decision::AcceptScalar,
                        ExecuteOutcome::Owned { owner_ordinal, .. } => {
                            Decision::AcceptOwned(owner_ordinal)
                        }
                        ExecuteOutcome::SemanticFailure { .. } => Decision::AcceptSemanticFailure,
                    };
                    (Some(response), decision)
                }
                Err(_) => (None, Decision::AbortMalformed),
            }
        };
        let decision = SettlementDecision {
            identity,
            decision: decision_kind,
        };
        transaction.decision_commit(&decision)?;
        let mut provider = provider.into_settlement()?;
        provider
            .decision_storage_mut()
            .copy_from_slice(transaction.decision_bytes());
        let settle_return = self.module_lease.invoke_settle(&mut provider)?;
        transaction
            .frame_storage_mut()
            .copy_from_slice(provider.frame_storage());
        transaction
            .candidate_storage_mut()
            .copy_from_slice(provider.candidate_storage());

        if settle_return != 0 {
            let interrupted_frame = RecoveryFrame::parse_reusing(
                transaction.frame_bytes(),
                &self.descriptor,
                settled_cells,
            )?;
            return Err(
                PrivateSettlementExecutionError::ProviderSettlementRejected {
                    code: settle_return,
                    phase: interrupted_frame.phase,
                    active_finalizers: interrupted_frame.active_finalizers,
                    next_action: interrupted_frame.next_action,
                    record_count: interrupted_frame.record_count,
                    frame_digest: frame_digest(transaction.frame_bytes()),
                },
            );
        }

        let settled_frame = RecoveryFrame::parse_reusing(
            transaction.frame_bytes(),
            &self.descriptor,
            settled_cells,
        )?;
        let candidate = CandidateReceipt::parse_reusing(
            transaction.candidate_bytes(),
            &self.descriptor,
            candidate_dispositions,
        )?;
        physical_actions(
            &self.descriptor,
            &settled_frame,
            &decision,
            &candidate,
            &mut actions,
            &mut replay_states,
        )?;
        transaction.provider_settled()?;
        candidate_bytes.copy_from_slice(transaction.candidate_bytes());
        let committed = transaction.receipt_commit(ReceiptCommitEvidence {
            request: &request,
            execute_return_code: execute_return,
            response_storage: ResponseStorageEvidence::Reserved,
            response: response.as_ref(),
            frame: &settled_frame,
            decision: &decision,
            actions: &actions,
            candidate: &candidate,
        })?;
        #[cfg(any(
            test,
            all(feature = "unstable-android-emulator-harness", target_os = "android"),
            all(
                feature = "unstable-ios-simulator-harness",
                target_os = "ios",
                target_abi = "sim"
            )
        ))]
        let _postcommit_allocation_count = postcommit_allocation_probe.finish();
        Ok(PrivateRecoveryCommit {
            identity,
            outcome: response.as_ref().map(|response| response.outcome),
            decision: decision_kind,
            candidate_bytes,
            committed,
        })
    }

    /// Commit ownership, deliberately skip the provider execute entry, then
    /// run the certified abort settlement from the unique start checkpoint.
    pub(crate) fn abort_host_unwind_before_execute(
        &self,
        arguments: &[PrivateSettlementArgumentV3],
    ) -> Result<PrivateHostUnwindCommit, PrivateSettlementExecutionError> {
        let mut transaction = self.reserve()?;
        let identity = transaction.identity();
        let (request, owners, payloads) = self.canonical_request(identity, arguments)?;
        transaction.stage_call(&request, &owners)?;
        let mut provider = self.module_lease.prepare_execute()?;
        let initial_frame = self.initial_host_unwind_frame(
            identity,
            &request,
            &payloads,
            provider.response_storage(),
        )?;
        let initial_frame_bytes = initial_frame.encode();
        provider
            .request_storage_mut()
            .copy_from_slice(transaction.request_bytes());
        provider
            .frame_storage_mut()
            .copy_from_slice(&initial_frame_bytes);
        transaction
            .frame_storage_mut()
            .copy_from_slice(&initial_frame_bytes);
        let mut candidate_bytes = vec![0; self.descriptor.capacities.candidate_receipt as usize];
        let mut actions = Vec::with_capacity(
            self.descriptor.capacities.resource_count.saturating_mul(2) as usize,
        );
        let mut replay_states =
            Vec::with_capacity(self.descriptor.capacities.resource_count as usize);
        let settled_cells = Vec::with_capacity(self.descriptor.capacities.resource_count as usize);
        let candidate_dispositions =
            Vec::with_capacity(self.descriptor.capacities.resource_count as usize);

        #[cfg(any(
            test,
            all(feature = "unstable-android-emulator-harness", target_os = "android"),
            all(
                feature = "unstable-ios-simulator-harness",
                target_os = "ios",
                target_abi = "sim"
            )
        ))]
        let postcommit_allocation_probe = crate::postcommit_allocation_probe::begin();
        transaction.call_commit()?;
        let decision = SettlementDecision {
            identity,
            decision: Decision::AbortHostUnwind,
        };
        transaction.decision_commit(&decision)?;
        let mut provider = provider.into_pre_execute_host_unwind_settlement()?;
        if provider.execute_return() != PRE_EXECUTE_HOST_UNWIND_CODE
            || provider.response_storage().iter().any(|byte| *byte != 0)
        {
            return Err(PrivateSettlementExecutionError::UnsupportedFixture);
        }
        transaction
            .execute_response_storage_mut()
            .copy_from_slice(provider.response_storage());
        provider
            .decision_storage_mut()
            .copy_from_slice(transaction.decision_bytes());
        let settle_return = self.module_lease.invoke_settle(&mut provider)?;
        transaction
            .frame_storage_mut()
            .copy_from_slice(provider.frame_storage());
        transaction
            .candidate_storage_mut()
            .copy_from_slice(provider.candidate_storage());
        if settle_return != 0 {
            let interrupted_frame = RecoveryFrame::parse_reusing(
                transaction.frame_bytes(),
                &self.descriptor,
                settled_cells,
            )?;
            return Err(
                PrivateSettlementExecutionError::ProviderSettlementRejected {
                    code: settle_return,
                    phase: interrupted_frame.phase,
                    active_finalizers: interrupted_frame.active_finalizers,
                    next_action: interrupted_frame.next_action,
                    record_count: interrupted_frame.record_count,
                    frame_digest: frame_digest(transaction.frame_bytes()),
                },
            );
        }
        let settled_frame = RecoveryFrame::parse_reusing(
            transaction.frame_bytes(),
            &self.descriptor,
            settled_cells,
        )?;
        let candidate = CandidateReceipt::parse_reusing(
            transaction.candidate_bytes(),
            &self.descriptor,
            candidate_dispositions,
        )?;
        physical_actions(
            &self.descriptor,
            &settled_frame,
            &decision,
            &candidate,
            &mut actions,
            &mut replay_states,
        )?;
        transaction.provider_settled()?;
        candidate_bytes.copy_from_slice(transaction.candidate_bytes());
        let committed = transaction.receipt_commit(ReceiptCommitEvidence {
            request: &request,
            execute_return_code: PRE_EXECUTE_HOST_UNWIND_CODE,
            response_storage: ResponseStorageEvidence::Reserved,
            response: None,
            frame: &settled_frame,
            decision: &decision,
            actions: &actions,
            candidate: &candidate,
        })?;
        #[cfg(any(
            test,
            all(feature = "unstable-android-emulator-harness", target_os = "android"),
            all(
                feature = "unstable-ios-simulator-harness",
                target_os = "ios",
                target_abi = "sim"
            )
        ))]
        let _postcommit_allocation_count = postcommit_allocation_probe.finish();
        Ok(PrivateHostUnwindCommit {
            identity,
            candidate_bytes,
            committed,
        })
    }

    pub(crate) fn reserve(
        &self,
    ) -> Result<SettlementTransaction<'_, SettlementInstanceLease>, SettlementLedgerError> {
        self.ledger.reserve()
    }

    pub(crate) fn replay_committed(
        &self,
        identity: RecoveryIdentity,
        candidate_bytes: &[u8],
    ) -> Result<CommittedResult, SettlementLedgerError> {
        self.ledger.replay_committed(identity, candidate_bytes)
    }

    pub(crate) fn is_poisoned(&self) -> bool {
        self.ledger.is_poisoned()
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.ledger.is_draining()
    }

    pub(crate) fn quarantined_count(&self) -> usize {
        self.ledger.quarantined_count()
    }

    pub(crate) fn module_instance_id(&self) -> semaprax_native_loader::ModuleInstanceId {
        self.module_lease.instance_id()
    }

    fn canonical_request(
        &self,
        identity: RecoveryIdentity,
        supplied: &[PrivateSettlementArgumentV3],
    ) -> Result<
        (ExecuteRequest, Vec<SettlementOwnerHandle>, Vec<u64>),
        PrivateSettlementExecutionError,
    > {
        if supplied.len() != self.descriptor.parameters.len() {
            return Err(PrivateSettlementExecutionError::UnsupportedFixture);
        }
        let mut arguments = Vec::with_capacity(self.descriptor.parameters.len());
        let resource_count = self.descriptor.capacities.resource_count as usize;
        let mut owners = vec![None; resource_count];
        let mut payloads = vec![None; resource_count];
        for (index, (parameter, supplied)) in
            self.descriptor.parameters.iter().zip(supplied).enumerate()
        {
            let argument = match (parameter, supplied) {
                (
                    Parameter::Scalar {
                        index: admitted,
                        kind: ScalarKind::I64,
                        ..
                    },
                    PrivateSettlementArgumentV3::I64(value),
                ) if *admitted == index => RequestArgument::I64 {
                    index: index as u32,
                    value: *value,
                },
                (
                    Parameter::Scalar {
                        index: admitted,
                        kind: ScalarKind::Bool,
                        ..
                    },
                    PrivateSettlementArgumentV3::Bool(value),
                ) if *admitted == index => RequestArgument::Bool {
                    index: index as u32,
                    value: *value,
                },
                (
                    Parameter::Owned {
                        index: admitted,
                        owner_ordinal,
                        ..
                    },
                    PrivateSettlementArgumentV3::Owned { handle, payload },
                ) if *admitted == index && *owner_ordinal < resource_count => {
                    if owners[*owner_ordinal].replace(*handle).is_some()
                        || payloads[*owner_ordinal].replace(*payload).is_some()
                    {
                        return Err(PrivateSettlementExecutionError::UnsupportedFixture);
                    }
                    RequestArgument::Owned {
                        index: index as u32,
                        owner_ordinal: *owner_ordinal as u32,
                        payload: *payload,
                    }
                }
                _ => return Err(PrivateSettlementExecutionError::UnsupportedFixture),
            };
            arguments.push(argument);
        }
        let owners = owners
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(PrivateSettlementExecutionError::UnsupportedFixture)?;
        let payloads = payloads
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(PrivateSettlementExecutionError::UnsupportedFixture)?;
        Ok((
            ExecuteRequest {
                identity: identity.call,
                arguments,
            },
            owners,
            payloads,
        ))
    }

    fn initial_frame(
        &self,
        identity: RecoveryIdentity,
        request: &ExecuteRequest,
        payloads: &[u64],
    ) -> Result<RecoveryFrame, PrivateSettlementExecutionError> {
        let start = self
            .descriptor
            .graph
            .starts
            .first()
            .and_then(|id| {
                self.descriptor
                    .graph
                    .checkpoints
                    .iter()
                    .find(|point| point.id == *id)
            })
            .ok_or(PrivateSettlementExecutionError::UnsupportedFixture)?;
        if self.descriptor.graph.starts.len() != 1
            || start.resources.len() != payloads.len()
            || start
                .resources
                .iter()
                .any(|state| *state != ResourceState::Live)
        {
            return Err(PrivateSettlementExecutionError::UnsupportedFixture);
        }
        let mut frame = RecoveryFrame {
            identity,
            request_digest: crate::callable_wire_v3::request_digest(&request.encode()),
            response_storage_digest: [0; 32],
            semantic_trace_digest: [0; 32],
            execute_return: ExecuteReturn::Pending,
            checkpoint: start.id,
            phase: FramePhase::Executing,
            decision_digest: [0; 32],
            next_action: 0,
            record_count: 0,
            active_finalizers: 0,
            cells: payloads
                .iter()
                .map(|payload| ResourceCell {
                    state: CellState::Live,
                    payload: *payload,
                })
                .collect(),
            action_chain_digest: [0; 32],
            pre_candidate_digest: [0; 32],
        };
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        Ok(frame)
    }

    fn initial_host_unwind_frame(
        &self,
        identity: RecoveryIdentity,
        request: &ExecuteRequest,
        payloads: &[u64],
        response_storage: &[u8],
    ) -> Result<RecoveryFrame, PrivateSettlementExecutionError> {
        if response_storage.len() != self.descriptor.capacities.execute_response as usize
            || response_storage.iter().any(|byte| *byte != 0)
        {
            return Err(PrivateSettlementExecutionError::UnsupportedFixture);
        }
        let mut frame = self.initial_frame(identity, request, payloads)?;
        frame.response_storage_digest = crate::callable_wire_v3::response_storage_digest(
            PRE_EXECUTE_HOST_UNWIND_CODE,
            response_storage,
        );
        frame.execute_return = ExecuteReturn::PreExecuteHostUnwind;
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        Ok(frame)
    }
}

fn physical_actions(
    descriptor: &Descriptor,
    frame: &RecoveryFrame,
    decision: &SettlementDecision,
    candidate: &CandidateReceipt,
    actions: &mut Vec<ActionRecord>,
    states: &mut Vec<CellState>,
) -> Result<(), WireError> {
    actions.clear();
    states.clear();
    let checkpoint = descriptor
        .graph
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.id == frame.checkpoint)
        .ok_or(WireError::ReplayMismatch)?;
    if checkpoint.resources.len() != frame.cells.len() {
        return Err(WireError::ReplayMismatch);
    }
    let cleanup = match decision.decision {
        Decision::AcceptScalar | Decision::AcceptSemanticFailure | Decision::AcceptOwned(_) => {
            &checkpoint.accept_order
        }
        Decision::AbortPhysical(_)
        | Decision::AbortMalformed
        | Decision::AbortTraceRejected
        | Decision::AbortHostUnwind => &checkpoint.abort_order,
    };
    for state in &checkpoint.resources {
        states.push(match state {
            ResourceState::Live => CellState::Live,
            ResourceState::ProvisionalResult => CellState::ProvisionalResult,
            ResourceState::Finalizing => CellState::Finalizing,
            ResourceState::Dead => CellState::Dead,
            ResourceState::Published => CellState::Published,
        });
    }
    let mut semantic_action_index = 0_u32;
    for owner in cleanup {
        let index = *owner as usize;
        let before = *states.get(index).ok_or(WireError::ReplayMismatch)?;
        let payload = frame
            .cells
            .get(index)
            .ok_or(WireError::ReplayMismatch)?
            .payload;
        if !matches!(before, CellState::Live | CellState::ProvisionalResult) {
            return Err(WireError::ReplayMismatch);
        }
        actions.push(ActionRecord {
            identity: frame.identity,
            semantic_action_index,
            boundary: ActionBoundary::Started,
            owner_ordinal: *owner,
            payload,
            before,
            after: CellState::Finalizing,
            checkpoint: frame.checkpoint,
        });
        actions.push(ActionRecord {
            identity: frame.identity,
            semantic_action_index,
            boundary: ActionBoundary::Completed,
            owner_ordinal: *owner,
            payload,
            before: CellState::Finalizing,
            after: CellState::Dead,
            checkpoint: frame.checkpoint,
        });
        states[index] = CellState::Dead;
        semantic_action_index = semantic_action_index
            .checked_add(1)
            .ok_or(WireError::CapacityMismatch)?;
    }
    if let Decision::AcceptOwned(owner) = decision.decision {
        let index = owner as usize;
        let state = states.get_mut(index).ok_or(WireError::ReplayMismatch)?;
        let payload = frame
            .cells
            .get(index)
            .ok_or(WireError::ReplayMismatch)?
            .payload;
        if *state != CellState::ProvisionalResult {
            return Err(WireError::ReplayMismatch);
        }
        actions.push(ActionRecord {
            identity: frame.identity,
            semantic_action_index,
            boundary: ActionBoundary::Publish,
            owner_ordinal: owner,
            payload,
            before: CellState::ProvisionalResult,
            after: CellState::Published,
            checkpoint: frame.checkpoint,
        });
        *state = CellState::Published;
        semantic_action_index = semantic_action_index
            .checked_add(1)
            .ok_or(WireError::CapacityMismatch)?;
    }
    let outcome_matches = match (candidate.outcome, decision.decision) {
        (CandidateOutcome::Scalar, Decision::AcceptScalar)
        | (CandidateOutcome::Failure, Decision::AcceptSemanticFailure)
        | (CandidateOutcome::Abort, Decision::AbortPhysical(_))
        | (CandidateOutcome::Abort, Decision::AbortMalformed)
        | (CandidateOutcome::Abort, Decision::AbortTraceRejected)
        | (CandidateOutcome::Abort, Decision::AbortHostUnwind) => true,
        (CandidateOutcome::Owned(owner), Decision::AcceptOwned(expected)) => owner == expected,
        _ => false,
    };
    if actions.len() != frame.record_count as usize
        || frame.next_action != semantic_action_index
        || states
            .iter()
            .zip(&frame.cells)
            .any(|(expected, actual)| *expected != actual.state)
        || !outcome_matches
    {
        return Err(WireError::ReplayMismatch);
    }
    Ok(())
}

fn parse_exact_admitted_descriptor(
    expected_descriptor: &[u8],
    mut admitted_matches: impl FnMut(&[u8]) -> bool,
    static_target: Option<IosStaticTarget>,
) -> Result<Descriptor, SettlementLedgerError> {
    if !admitted_matches(expected_descriptor) {
        return Err(SettlementLedgerError::DescriptorMismatch);
    }
    match static_target {
        Some(target) => Descriptor::parse_for_target(
            expected_descriptor,
            target.canonical_tag(),
            Linkage::IosStatic,
        ),
        None => Descriptor::parse(expected_descriptor),
    }
    .map_err(|_| SettlementLedgerError::DescriptorMismatch)
}

#[cfg(test)]
mod tests {
    use semaprax::codegen::emit_native_callable_v3_descriptor;
    use semaprax::hir::DeclarationId;
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::*;

    #[test]
    fn canonical_same_capacity_descriptor_substitution_fails_exact_binding() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let image_a = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.discard"),
        )
        .unwrap();
        let image_b = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.identity"),
        )
        .unwrap();
        let parsed_a = Descriptor::parse(image_a.bytes()).unwrap();
        let parsed_b = Descriptor::parse(image_b.bytes()).unwrap();
        assert_eq!(parsed_a.capacities.request, parsed_b.capacities.request);
        assert_eq!(
            parsed_a.capacities.execute_response,
            parsed_b.capacities.execute_response
        );
        assert_eq!(parsed_a.capacities.frame, parsed_b.capacities.frame);
        assert_eq!(parsed_a.capacities.decision, parsed_b.capacities.decision);
        assert_eq!(
            parsed_a.capacities.candidate_receipt,
            parsed_b.capacities.candidate_receipt
        );
        assert_ne!(image_a.bytes(), image_b.bytes());
        assert_eq!(
            parse_exact_admitted_descriptor(
                image_b.bytes(),
                |candidate| candidate == image_a.bytes(),
                None,
            ),
            Err(SettlementLedgerError::DescriptorMismatch)
        );
    }
}
