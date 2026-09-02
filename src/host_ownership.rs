//! Target-neutral ownership transaction model for future safe host adapters.
//!
//! Generated native resource entry points remain gated by `SPX-B104`. This
//! private reference model fixes the boundary semantics that an adapter must
//! implement first: registry-issued authority, trusted call contracts, fully
//! atomic `own` ingress, and an unambiguous split between rejection and an
//! executed language result. It contains no raw pointers, caller arenas, slot
//! reuse, imported finalizers, or public payload-adoption API.

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};

pub(crate) const HOST_OWNERSHIP_SCHEMA_V1: &str = "semaprax.host-ownership.v1";

static NEXT_REGISTRY_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HostIdentity(String);

impl HostIdentity {
    pub(crate) fn try_new(value: impl Into<String>) -> Result<Self, HostBoundaryRejection> {
        let value = value.into();
        if value.is_empty() || value.contains('\0') {
            return Err(HostBoundaryRejection::InvalidIdentity);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostResourceProvenance {
    module: HostIdentity,
    adapter: HostIdentity,
    resource_type: HostIdentity,
    lifecycle: HostIdentity,
    owner_thread: u64,
}

impl HostResourceProvenance {
    pub(crate) fn try_new(
        module: HostIdentity,
        adapter: HostIdentity,
        resource_type: HostIdentity,
        lifecycle: HostIdentity,
        owner_thread: u64,
    ) -> Result<Self, HostBoundaryRejection> {
        if owner_thread == 0 {
            return Err(HostBoundaryRejection::WrongThread);
        }
        Ok(Self {
            module,
            adapter,
            resource_type,
            lifecycle,
            owner_thread,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostResourceRequirement {
    resource_type: HostIdentity,
    lifecycle: HostIdentity,
}

impl HostResourceRequirement {
    pub(crate) fn new(resource_type: HostIdentity, lifecycle: HostIdentity) -> Self {
        Self {
            resource_type,
            lifecycle,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostResultPlan {
    Scalar,
    OwnedInput { input_index: usize },
}

/// Immutable compiler/adapter metadata for one exported function.
///
/// An external host caller supplies only opaque owner tokens. The trusted shim
/// constructs this contract and observes the executing thread; neither value
/// is part of the eventual untrusted host input surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCallContract {
    module: HostIdentity,
    adapter: HostIdentity,
    function: HostIdentity,
    bound_thread: u64,
    inputs: Vec<HostResourceRequirement>,
    result: HostResultPlan,
}

impl HostCallContract {
    pub(crate) fn try_new(
        module: HostIdentity,
        adapter: HostIdentity,
        function: HostIdentity,
        bound_thread: u64,
        inputs: Vec<HostResourceRequirement>,
        result: HostResultPlan,
    ) -> Result<Self, HostBoundaryRejection> {
        if bound_thread == 0 {
            return Err(HostBoundaryRejection::WrongThread);
        }
        if let HostResultPlan::OwnedInput { input_index } = result {
            if input_index >= inputs.len() {
                return Err(HostBoundaryRejection::InvalidOwnedResult);
            }
        }
        Ok(Self {
            module,
            adapter,
            function,
            bound_thread,
            inputs,
            result,
        })
    }

    pub(crate) fn function(&self) -> &HostIdentity {
        &self.function
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HostOwnerToken {
    registry_nonce: u64,
    slot: u64,
    generation: u64,
}

impl HostOwnerToken {
    /// Nonzero ledger slot authenticated by the physical host credential.
    pub(crate) const fn slot(self) -> u64 {
        self.slot
    }

    /// Nonzero generation authenticated by the physical host credential.
    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact next-generation token expected from owned-result publication.
    pub(crate) const fn next_generation(self) -> Option<Self> {
        match self.generation.checked_add(1) {
            Some(generation) => Some(Self {
                registry_nonce: self.registry_nonce,
                slot: self.slot,
                generation,
            }),
            None => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCallRequest {
    contract: HostCallContract,
    executing_thread: u64,
    owners: Vec<HostOwnerToken>,
}

impl HostCallRequest {
    pub(crate) fn new(
        contract: HostCallContract,
        executing_thread: u64,
        owners: Vec<HostOwnerToken>,
    ) -> Self {
        Self {
            contract,
            executing_thread,
            owners,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostPublishedValue {
    Scalar(i64),
    Owner(HostOwnerToken),
}

/// Only committed executions produce this type. Rejection exists solely in
/// [`HostCallStart`], before an invocation handle is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostCallOutcome {
    ExecutedSuccess(HostPublishedValue),
    ExecutedFailure(NormalizedStatus),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostBoundaryResult {
    Rejected(HostBoundaryRejection),
    Executed(HostCallOutcome),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostBoundaryRejection {
    InvalidIdentity,
    RegistryExhausted,
    InvocationExhausted,
    ReentrantInvocation,
    RegistryPoisoned,
    InputCountMismatch,
    UnknownOwner,
    StaleOwner,
    OwnerNotLive,
    DuplicateOwner,
    WrongModule,
    WrongAdapter,
    WrongResourceType,
    WrongLifecycle,
    WrongThread,
    InvalidOwnedResult,
    ResultKindMismatch,
    StalePlan,
}

enum HostCallStart<'a> {
    Rejected(HostBoundaryRejection),
    Committed(HostCommittedCall<'a>),
}

enum HostCommittedCall<'a> {
    Scalar(HostScalarInvocation<'a>),
    Owned(HostOwnedInvocation<'a>),
}

/// Read-only physical payload view for the trusted generated executor.
///
/// The integer may necessarily cross into generated target code, so this type
/// is not physical non-escape enforcement. Only the audited backend/adapter may
/// receive the execution closure; untrusted hosts receive owner tokens.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostCommittedResource {
    payload: u64,
}

impl HostCommittedResource {
    pub(crate) const fn payload(&self) -> u64 {
        self.payload
    }
}

/// Detached linear invocation state for a physical host that must execute
/// trusted code without holding a borrow of its owner registry.
///
/// Construction commits every input atomically. The holder must then call one
/// matching completion method or [`HostOwnershipRegistry::abandon_prepared`].
#[must_use = "a prepared host invocation must be completed or abandoned"]
#[allow(
    dead_code,
    reason = "used by the unpublished physical host through audited source inclusion"
)]
pub(crate) struct HostPreparedInvocation {
    sequence: u64,
    result_slot: Option<u64>,
    resources: Vec<HostCommittedResource>,
    payloads: Vec<u64>,
}

/// Fully allocated, non-mutating detached ingress plan.
///
/// The plan is registry- and sequence-bound. A physical host may serialize
/// payloads and allocate its complete response storage from this value before
/// [`HostOwnershipRegistry::commit_plan`] changes any owner state.
#[must_use = "a planned host invocation must be committed or discarded"]
pub(crate) struct HostPlannedInvocation {
    registry_nonce: u64,
    sequence: u64,
    result_slot: Option<u64>,
    tokens: Vec<HostOwnerToken>,
    slots: Vec<u64>,
    resources: Vec<HostCommittedResource>,
    payloads: Vec<u64>,
}

impl HostPlannedInvocation {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) fn payloads(&self) -> &[u64] {
        &self.payloads
    }
}

#[allow(
    dead_code,
    reason = "used by the unpublished physical host through audited source inclusion"
)]
impl HostPreparedInvocation {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) fn payloads(&self) -> &[u64] {
        &self.payloads
    }
}

/// Registry-bound completion guard kept inside `execute_scalar`.
struct HostScalarInvocation<'a> {
    registry: &'a mut HostOwnershipRegistry,
    sequence: u64,
    resources: Vec<HostCommittedResource>,
    completed: bool,
}

impl HostScalarInvocation<'_> {
    fn run<F>(mut self, execute: F) -> HostCallOutcome
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<i64, NormalizedStatus>,
    {
        let result = execute(&self.resources);
        let outcome = match result {
            Ok(value) => self.registry.finish(self.sequence, None, Some(value), None),
            Err(status) => self
                .registry
                .finish(self.sequence, None, None, Some(status)),
        };
        self.completed = true;
        outcome
    }
}

impl Drop for HostScalarInvocation<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.registry.abandon(self.sequence);
            self.completed = true;
        }
    }
}

/// Linear completion capability for an owned result. The selected result slot
/// was fixed by the trusted call contract before commit.
struct HostOwnedInvocation<'a> {
    registry: &'a mut HostOwnershipRegistry,
    sequence: u64,
    result_slot: u64,
    resources: Vec<HostCommittedResource>,
    completed: bool,
}

impl HostOwnedInvocation<'_> {
    fn run<F>(mut self, execute: F) -> HostCallOutcome
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<(), NormalizedStatus>,
    {
        let result = execute(&self.resources);
        let outcome = match result {
            Ok(()) => self
                .registry
                .finish(self.sequence, Some(self.result_slot), None, None),
            Err(status) => self
                .registry
                .finish(self.sequence, None, None, Some(status)),
        };
        self.completed = true;
        outcome
    }
}

impl Drop for HostOwnedInvocation<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.registry.abandon(self.sequence);
            self.completed = true;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostOwnerState {
    Live,
    InInvocation(u64),
    Dead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostOwnerEntry {
    generation: u64,
    provenance: HostResourceProvenance,
    payload: u64,
    state: HostOwnerState,
}

#[derive(Debug, Eq, PartialEq)]
struct ActiveInvocation {
    sequence: u64,
    slots: Vec<u64>,
}

/// Single-threaded reference ledger for a future adapter implementation.
///
/// The registry is deliberately non-`Clone`. Its nonce is allocated uniquely
/// within this linked runtime instance,
/// and a committed call returns a linear capability borrowing this exact
/// registry. A native implementation may use locks/atomics instead, but it
/// must preserve these observable transaction semantics.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostOwnershipRegistry {
    schema: &'static str,
    nonce: u64,
    next_slot: u64,
    next_invocation: u64,
    owners: BTreeMap<u64, HostOwnerEntry>,
    active: Option<ActiveInvocation>,
    last_abandonment: bool,
    poisoned: bool,
}

impl HostOwnershipRegistry {
    pub(crate) fn try_new() -> Result<Self, HostBoundaryRejection> {
        let nonce = NEXT_REGISTRY_NONCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| HostBoundaryRejection::RegistryExhausted)?;
        Ok(Self {
            schema: HOST_OWNERSHIP_SCHEMA_V1,
            nonce,
            next_slot: 1,
            next_invocation: 1,
            owners: BTreeMap::new(),
            active: None,
            last_abandonment: false,
            poisoned: false,
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Register an owner created by an already validated adapter/import.
    /// There is intentionally no public raw-payload adoption path.
    pub(crate) fn register_adapter_owner(
        &mut self,
        provenance: HostResourceProvenance,
        payload: u64,
    ) -> Result<HostOwnerToken, HostBoundaryRejection> {
        if self.poisoned {
            return Err(HostBoundaryRejection::RegistryPoisoned);
        }
        if self.active.is_some() {
            return Err(HostBoundaryRejection::ReentrantInvocation);
        }
        let slot = self.next_slot;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or(HostBoundaryRejection::RegistryExhausted)?;
        let generation = 1;
        self.owners.insert(
            slot,
            HostOwnerEntry {
                generation,
                provenance,
                payload,
                state: HostOwnerState::Live,
            },
        );
        Ok(HostOwnerToken {
            registry_nonce: self.nonce,
            slot,
            generation,
        })
    }

    /// Exactly undo the immediately preceding adapter-owner registration.
    ///
    /// This private rollback exists only so the physical host can mint and
    /// authenticate the capability after it learns the assigned slot without
    /// stranding a live ledger owner if that fallible step fails. Same-thread
    /// exclusive access makes intervening registration impossible.
    pub(crate) fn rollback_adapter_owner(
        &mut self,
        token: HostOwnerToken,
    ) -> Result<(), HostBoundaryRejection> {
        if self.poisoned || self.active.is_some() {
            return Err(HostBoundaryRejection::StalePlan);
        }
        let expected_next = token
            .slot
            .checked_add(1)
            .ok_or(HostBoundaryRejection::StalePlan)?;
        if token.registry_nonce != self.nonce || self.next_slot != expected_next {
            return Err(HostBoundaryRejection::StalePlan);
        }
        let owner = self
            .owners
            .get(&token.slot)
            .ok_or(HostBoundaryRejection::StalePlan)?;
        if owner.generation != token.generation || owner.state != HostOwnerState::Live {
            return Err(HostBoundaryRejection::StalePlan);
        }
        self.owners
            .remove(&token.slot)
            .expect("validated rollback owner remains present");
        self.next_slot = token.slot;
        Ok(())
    }

    /// Execute a scalar-result call inside a must-complete registry scope.
    ///
    /// If generated execution unwinds, the private completion guard consumes
    /// every committed input, clears the active invocation, records a stable
    /// adapter failure, and then allows the original panic to continue.
    pub(crate) fn execute_scalar<F>(
        &mut self,
        request: HostCallRequest,
        execute: F,
    ) -> HostBoundaryResult
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<i64, NormalizedStatus>,
    {
        if request.contract.result != HostResultPlan::Scalar {
            return HostBoundaryResult::Rejected(HostBoundaryRejection::ResultKindMismatch);
        }
        match self.begin_call(request) {
            HostCallStart::Rejected(rejection) => HostBoundaryResult::Rejected(rejection),
            HostCallStart::Committed(HostCommittedCall::Scalar(invocation)) => {
                HostBoundaryResult::Executed(invocation.run(execute))
            }
            HostCallStart::Committed(HostCommittedCall::Owned(_)) => {
                unreachable!("scalar result plan creates a scalar completion guard")
            }
        }
    }

    /// Execute an owned-result call inside a must-complete registry scope.
    pub(crate) fn execute_owned<F>(
        &mut self,
        request: HostCallRequest,
        execute: F,
    ) -> HostBoundaryResult
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<(), NormalizedStatus>,
    {
        if !matches!(request.contract.result, HostResultPlan::OwnedInput { .. }) {
            return HostBoundaryResult::Rejected(HostBoundaryRejection::ResultKindMismatch);
        }
        match self.begin_call(request) {
            HostCallStart::Rejected(rejection) => HostBoundaryResult::Rejected(rejection),
            HostCallStart::Committed(HostCommittedCall::Owned(invocation)) => {
                HostBoundaryResult::Executed(invocation.run(execute))
            }
            HostCallStart::Committed(HostCommittedCall::Scalar(_)) => {
                unreachable!("owned result plan creates an owned completion guard")
            }
        }
    }

    /// Validate and atomically commit a scalar call while returning detached
    /// execution state. No registry borrow needs to remain live while trusted
    /// adapter code runs.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn prepare_scalar(
        &mut self,
        request: HostCallRequest,
    ) -> Result<HostPreparedInvocation, HostBoundaryRejection> {
        if request.contract.result != HostResultPlan::Scalar {
            return Err(HostBoundaryRejection::ResultKindMismatch);
        }
        self.prepare_call(request)
    }

    /// Validate and atomically commit an owned-result call while returning
    /// detached execution state.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn prepare_owned(
        &mut self,
        request: HostCallRequest,
    ) -> Result<HostPreparedInvocation, HostBoundaryRejection> {
        if !matches!(request.contract.result, HostResultPlan::OwnedInput { .. }) {
            return Err(HostBoundaryRejection::ResultKindMismatch);
        }
        self.prepare_call(request)
    }

    /// Allocate and validate a scalar invocation without consuming any owner.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn plan_scalar(
        &self,
        request: &HostCallRequest,
    ) -> Result<HostPlannedInvocation, HostBoundaryRejection> {
        if request.contract.result != HostResultPlan::Scalar {
            return Err(HostBoundaryRejection::ResultKindMismatch);
        }
        self.plan_call(request)
    }

    /// Allocate and validate an owned-result invocation without consuming any
    /// owner.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn plan_owned(
        &self,
        request: &HostCallRequest,
    ) -> Result<HostPlannedInvocation, HostBoundaryRejection> {
        if !matches!(request.contract.result, HostResultPlan::OwnedInput { .. }) {
            return Err(HostBoundaryRejection::ResultKindMismatch);
        }
        self.plan_call(request)
    }

    /// Atomically commit an already fully allocated ingress plan.
    ///
    /// Validation is allocation-free and completes before the first owner
    /// mutation. A stale or foreign plan is rejected without changing the
    /// registry.
    pub(crate) fn commit_plan(
        &mut self,
        plan: HostPlannedInvocation,
    ) -> Result<HostPreparedInvocation, HostBoundaryRejection> {
        self.validate_plan(&plan)?;
        let HostPlannedInvocation {
            registry_nonce: _,
            sequence,
            result_slot,
            tokens,
            slots,
            resources,
            payloads,
        } = plan;

        self.next_invocation += 1;
        for token in &tokens {
            self.owners
                .get_mut(&token.slot)
                .expect("plan validation proved every owner exists")
                .state = HostOwnerState::InInvocation(sequence);
        }
        self.active = Some(ActiveInvocation { sequence, slots });
        Ok(HostPreparedInvocation {
            sequence,
            result_slot,
            resources,
            payloads,
        })
    }

    /// Validate a prospective detached invocation without mutating registry
    /// state and return the exact sequence that a subsequent matching prepare
    /// will commit. A same-thread host uses this to finish allocating and
    /// serializing its physical call before it consumes any owner.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn preflight_prepared(
        &self,
        request: &HostCallRequest,
    ) -> Result<u64, HostBoundaryRejection> {
        self.preflight(request)?;
        Ok(self.next_invocation)
    }

    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn complete_prepared_scalar(
        &mut self,
        invocation: HostPreparedInvocation,
        result: Result<i64, NormalizedStatus>,
    ) -> HostCallOutcome {
        self.complete_prepared_scalar_ref(&invocation, result)
    }

    pub(crate) fn complete_prepared_scalar_ref(
        &mut self,
        invocation: &HostPreparedInvocation,
        result: Result<i64, NormalizedStatus>,
    ) -> HostCallOutcome {
        assert!(
            invocation.result_slot.is_none(),
            "scalar completion requires a scalar prepared invocation"
        );
        match result {
            Ok(value) => self.finish(invocation.sequence, None, Some(value), None),
            Err(status) => self.finish(invocation.sequence, None, None, Some(status)),
        }
    }

    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn complete_prepared_owned(
        &mut self,
        invocation: HostPreparedInvocation,
        result: Result<(), NormalizedStatus>,
    ) -> HostCallOutcome {
        self.complete_prepared_owned_ref(&invocation, result)
    }

    pub(crate) fn complete_prepared_owned_ref(
        &mut self,
        invocation: &HostPreparedInvocation,
        result: Result<(), NormalizedStatus>,
    ) -> HostCallOutcome {
        let result_slot = invocation
            .result_slot
            .expect("owned completion requires an owned prepared invocation");
        match result {
            Ok(()) => self.finish(invocation.sequence, Some(result_slot), None, None),
            Err(status) => self.finish(invocation.sequence, None, None, Some(status)),
        }
    }

    /// Complete an owned-result invocation only if its exact next-generation
    /// publication token is still provable before any ledger mutation.
    ///
    /// A mismatch leaves the committed invocation active so its RAII holder
    /// can abandon it without orphaning a live owner.
    pub(crate) fn complete_prepared_owned_expected_ref(
        &mut self,
        invocation: &HostPreparedInvocation,
        expected_owner: HostOwnerToken,
    ) -> Result<HostCallOutcome, HostBoundaryRejection> {
        let result_slot = invocation
            .result_slot
            .ok_or(HostBoundaryRejection::ResultKindMismatch)?;
        self.finish_checked(invocation.sequence, result_slot, expected_owner)
    }

    /// Consume every committed input after trusted execution unwinds.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn abandon_prepared(&mut self, invocation: HostPreparedInvocation) {
        self.abandon_prepared_ref(&invocation);
    }

    pub(crate) fn abandon_prepared_ref(&mut self, invocation: &HostPreparedInvocation) {
        self.abandon(invocation.sequence);
    }

    pub(crate) fn take_last_abandonment(&mut self) -> Option<NormalizedStatus> {
        if self.take_last_abandonment_flag() {
            Some(abandonment_status())
        } else {
            None
        }
    }

    /// Allocation-free acknowledgement used when the physical host prepared
    /// the normalized adapter status before ownership commit.
    pub(crate) fn take_last_abandonment_flag(&mut self) -> bool {
        std::mem::take(&mut self.last_abandonment)
    }

    /// Construct the canonical abandonment status while the call is still in
    /// its fallible precommit phase.
    #[allow(
        dead_code,
        reason = "used by the unpublished physical host through audited source inclusion"
    )]
    pub(crate) fn prepare_abandonment_status() -> NormalizedStatus {
        abandonment_status()
    }

    /// Validate every fallible condition, allocate execution views, and then
    /// commit all owners together. No allocation occurs after the first owner
    /// state changes.
    fn begin_call(&mut self, request: HostCallRequest) -> HostCallStart<'_> {
        let result = request.contract.result;
        let prepared = match self.prepare_call(request) {
            Ok(prepared) => prepared,
            Err(rejection) => return HostCallStart::Rejected(rejection),
        };
        let HostPreparedInvocation {
            sequence,
            result_slot,
            resources,
            payloads: _,
        } = prepared;

        match result {
            HostResultPlan::Scalar => {
                HostCallStart::Committed(HostCommittedCall::Scalar(HostScalarInvocation {
                    registry: self,
                    sequence,
                    resources,
                    completed: false,
                }))
            }
            HostResultPlan::OwnedInput { .. } => {
                HostCallStart::Committed(HostCommittedCall::Owned(HostOwnedInvocation {
                    registry: self,
                    sequence,
                    result_slot: result_slot.expect("owned result slot was computed before commit"),
                    resources,
                    completed: false,
                }))
            }
        }
    }

    /// Validate every fallible condition, allocate both internal and physical
    /// execution views, and only then mutate owner state.
    fn prepare_call(
        &mut self,
        request: HostCallRequest,
    ) -> Result<HostPreparedInvocation, HostBoundaryRejection> {
        let plan = self.plan_call(&request)?;
        self.commit_plan(plan)
    }

    fn plan_call(
        &self,
        request: &HostCallRequest,
    ) -> Result<HostPlannedInvocation, HostBoundaryRejection> {
        self.preflight(request)?;
        let tokens = request.owners.clone();
        let slots = request
            .owners
            .iter()
            .map(|owner| owner.slot)
            .collect::<Vec<_>>();
        let resources = request
            .owners
            .iter()
            .map(|token| HostCommittedResource {
                payload: self
                    .lookup(*token)
                    .expect("preflight proved every owner exists")
                    .payload,
            })
            .collect::<Vec<_>>();
        let payloads = resources
            .iter()
            .map(HostCommittedResource::payload)
            .collect::<Vec<_>>();
        let result_slot = match request.contract.result {
            HostResultPlan::Scalar => None,
            HostResultPlan::OwnedInput { input_index } => Some(slots[input_index]),
        };
        Ok(HostPlannedInvocation {
            registry_nonce: self.nonce,
            sequence: self.next_invocation,
            result_slot,
            tokens,
            slots,
            resources,
            payloads,
        })
    }

    fn validate_plan(&self, plan: &HostPlannedInvocation) -> Result<(), HostBoundaryRejection> {
        if self.poisoned
            || self.active.is_some()
            || plan.registry_nonce != self.nonce
            || plan.sequence != self.next_invocation
            || plan.tokens.len() != plan.slots.len()
            || plan.tokens.len() != plan.resources.len()
            || plan.tokens.len() != plan.payloads.len()
        {
            return Err(HostBoundaryRejection::StalePlan);
        }
        for index in 0..plan.tokens.len() {
            let token = plan.tokens[index];
            if token.slot != plan.slots[index] {
                return Err(HostBoundaryRejection::StalePlan);
            }
            let owner = self
                .lookup(token)
                .map_err(|_| HostBoundaryRejection::StalePlan)?;
            if owner.state != HostOwnerState::Live
                || owner.payload != plan.payloads[index]
                || owner.payload != plan.resources[index].payload()
            {
                return Err(HostBoundaryRejection::StalePlan);
            }
        }
        if let Some(result_slot) = plan.result_slot {
            if !plan.slots.contains(&result_slot) {
                return Err(HostBoundaryRejection::StalePlan);
            }
        }
        Ok(())
    }

    pub(crate) fn is_live(&self, token: HostOwnerToken) -> bool {
        self.lookup(token)
            .is_ok_and(|owner| owner.state == HostOwnerState::Live)
    }

    pub(crate) fn payload(&self, token: HostOwnerToken) -> Option<u64> {
        self.lookup(token)
            .ok()
            .filter(|owner| owner.state == HostOwnerState::Live)
            .map(|owner| owner.payload)
    }

    /// Retire one still-live owner outside an invocation that owns it.
    ///
    /// The unpublished physical host calls this from its noncopying owner
    /// guard. The current admitted slice contains only trivial finalizers, so
    /// retirement clears logical liveness and returns the opaque payload for
    /// audit evidence without invoking foreign code.
    pub(crate) fn retire_owner(
        &mut self,
        token: HostOwnerToken,
    ) -> Result<u64, HostBoundaryRejection> {
        if self.poisoned {
            return Err(HostBoundaryRejection::RegistryPoisoned);
        }
        let owner = self.lookup(token)?;
        if owner.state != HostOwnerState::Live {
            return Err(HostBoundaryRejection::OwnerNotLive);
        }
        let owner = self
            .owners
            .get_mut(&token.slot)
            .expect("lookup proved the owner slot exists");
        owner.state = HostOwnerState::Dead;
        Ok(owner.payload)
    }

    /// Number of currently live owner generations, excluding committed calls.
    pub(crate) fn live_owner_count(&self) -> usize {
        self.owners
            .values()
            .filter(|owner| owner.state == HostOwnerState::Live)
            .count()
    }

    fn preflight(&self, request: &HostCallRequest) -> Result<(), HostBoundaryRejection> {
        if self.poisoned {
            return Err(HostBoundaryRejection::RegistryPoisoned);
        }
        if self.active.is_some() {
            return Err(HostBoundaryRejection::ReentrantInvocation);
        }
        if self.next_invocation == u64::MAX {
            return Err(HostBoundaryRejection::InvocationExhausted);
        }
        if request.executing_thread == 0
            || request.executing_thread != request.contract.bound_thread
        {
            return Err(HostBoundaryRejection::WrongThread);
        }
        if request.owners.len() != request.contract.inputs.len() {
            return Err(HostBoundaryRejection::InputCountMismatch);
        }

        let mut unique = BTreeSet::new();
        for (token, requirement) in request.owners.iter().zip(&request.contract.inputs) {
            if !unique.insert(*token) {
                return Err(HostBoundaryRejection::DuplicateOwner);
            }
            let owner = self.lookup(*token)?;
            if owner.state != HostOwnerState::Live {
                return Err(HostBoundaryRejection::OwnerNotLive);
            }
            compare_contract(
                &owner.provenance,
                &request.contract,
                requirement,
                request.executing_thread,
            )?;
        }

        if let HostResultPlan::OwnedInput { input_index } = request.contract.result {
            let result = self.lookup(request.owners[input_index])?;
            if result.generation == u64::MAX {
                return Err(HostBoundaryRejection::RegistryExhausted);
            }
        }
        Ok(())
    }

    fn lookup(&self, token: HostOwnerToken) -> Result<&HostOwnerEntry, HostBoundaryRejection> {
        if token.registry_nonce != self.nonce {
            return Err(HostBoundaryRejection::UnknownOwner);
        }
        let owner = self
            .owners
            .get(&token.slot)
            .ok_or(HostBoundaryRejection::UnknownOwner)?;
        if owner.generation != token.generation {
            return Err(HostBoundaryRejection::StaleOwner);
        }
        Ok(owner)
    }

    fn finish(
        &mut self,
        sequence: u64,
        published_slot: Option<u64>,
        scalar: Option<i64>,
        failure: Option<NormalizedStatus>,
    ) -> HostCallOutcome {
        let active = self
            .active
            .as_ref()
            .expect("linear invocation capability requires one active call");
        assert_eq!(
            active.sequence, sequence,
            "linear invocation capability is registry-bound"
        );
        assert!(
            matches!(
                (&published_slot, &scalar, &failure),
                (None, Some(_), None) | (Some(_), None, None) | (None, None, Some(_))
            ),
            "typed invocation completion has one exact outcome"
        );
        if let Some(slot) = published_slot {
            assert!(
                active.slots.contains(&slot),
                "owned completion publishes one committed input"
            );
        }
        for slot in &active.slots {
            let owner = self
                .owners
                .get(slot)
                .expect("committed owners cannot disappear");
            assert_eq!(
                owner.state,
                HostOwnerState::InInvocation(sequence),
                "committed owner state is immutable until completion"
            );
            if Some(*slot) == published_slot {
                assert!(
                    owner.generation < u64::MAX,
                    "owned-result generation overflow was rejected before commit"
                );
            }
        }

        let active = self
            .active
            .take()
            .expect("prevalidated active invocation cannot disappear");

        let mut published_owner = None;
        for slot in active.slots {
            let owner = self
                .owners
                .get_mut(&slot)
                .expect("prevalidated committed owner cannot disappear");
            if Some(slot) == published_slot {
                owner.generation += 1;
                owner.state = HostOwnerState::Live;
                published_owner = Some(HostOwnerToken {
                    registry_nonce: self.nonce,
                    slot,
                    generation: owner.generation,
                });
            } else {
                owner.state = HostOwnerState::Dead;
            }
        }

        if let Some(status) = failure {
            return HostCallOutcome::ExecutedFailure(status);
        }
        match (scalar, published_owner) {
            (Some(value), None) => {
                HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(value))
            }
            (None, Some(owner)) => {
                HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(owner))
            }
            _ => unreachable!("completion outcome was prevalidated before mutation"),
        }
    }

    fn finish_checked(
        &mut self,
        sequence: u64,
        published_slot: u64,
        expected_owner: HostOwnerToken,
    ) -> Result<HostCallOutcome, HostBoundaryRejection> {
        let active = self
            .active
            .as_ref()
            .ok_or(HostBoundaryRejection::StalePlan)?;
        if active.sequence != sequence
            || !active.slots.contains(&published_slot)
            || expected_owner.registry_nonce != self.nonce
            || expected_owner.slot != published_slot
        {
            return Err(HostBoundaryRejection::StalePlan);
        }
        for slot in &active.slots {
            let owner = self
                .owners
                .get(slot)
                .ok_or(HostBoundaryRejection::StalePlan)?;
            if owner.state != HostOwnerState::InInvocation(sequence) {
                return Err(HostBoundaryRejection::StalePlan);
            }
            if *slot == published_slot
                && owner.generation.checked_add(1) != Some(expected_owner.generation)
            {
                return Err(HostBoundaryRejection::StalePlan);
            }
        }

        let active = self.active.take().ok_or(HostBoundaryRejection::StalePlan)?;
        for slot in active.slots {
            let owner = self
                .owners
                .get_mut(&slot)
                .ok_or(HostBoundaryRejection::StalePlan)?;
            if slot == published_slot {
                owner.generation = expected_owner.generation;
                owner.state = HostOwnerState::Live;
            } else {
                owner.state = HostOwnerState::Dead;
            }
        }
        Ok(HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(
            expected_owner,
        )))
    }

    fn abandon(&mut self, sequence: u64) {
        let Some(active) = self.active.take() else {
            self.poisoned = true;
            self.last_abandonment = true;
            return;
        };
        if active.sequence != sequence {
            self.poisoned = true;
        }
        for slot in active.slots {
            match self.owners.get_mut(&slot) {
                Some(owner) => {
                    if owner.state != HostOwnerState::InInvocation(sequence) {
                        self.poisoned = true;
                    }
                    owner.state = HostOwnerState::Dead;
                }
                None => self.poisoned = true,
            }
        }
        self.last_abandonment = true;
    }
}

fn abandonment_status() -> NormalizedStatus {
    NormalizedStatus::try_new(
        "semaprax.adapter.host-ownership.v1",
        1,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("the fixed host-ownership abandonment status is valid")
}

fn compare_contract(
    actual: &HostResourceProvenance,
    contract: &HostCallContract,
    requirement: &HostResourceRequirement,
    executing_thread: u64,
) -> Result<(), HostBoundaryRejection> {
    if actual.module != contract.module {
        return Err(HostBoundaryRejection::WrongModule);
    }
    if actual.adapter != contract.adapter {
        return Err(HostBoundaryRejection::WrongAdapter);
    }
    if actual.resource_type != requirement.resource_type {
        return Err(HostBoundaryRejection::WrongResourceType);
    }
    if actual.lifecycle != requirement.lifecycle {
        return Err(HostBoundaryRejection::WrongLifecycle);
    }
    if actual.owner_thread != executing_thread {
        return Err(HostBoundaryRejection::WrongThread);
    }
    Ok(())
}

#[cfg(test)]
#[path = "host_ownership/tests.rs"]
mod tests;
